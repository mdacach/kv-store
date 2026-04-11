//! Discrete-event simulator for the key-value store.
//!
//! Drives the server state machine via a priority-queue event loop.
//! Client workload is managed internally by the simulator: each client has a
//! queue of operations and at most one outstanding request at a time
//! ("stop-and-wait").

mod event;

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::history::History;
use crate::linearizability_checker::{self, CheckResult};
use crate::node::Operation;
use crate::server::Server;
use crate::{ActorId, ClientID, Message, MessagePayload, OperationID, StateMachine};

use event::{Event, EventQueue};

/// Per-client workload state, managed by the simulator.
///
/// Clients use **stop-and-wait**: a client sends one request, waits for
/// the response, and only then sends the next.
struct ClientState {
    /// Operations remaining to be sent, in order.
    operations: VecDeque<Operation>,
    /// The operation currently awaiting a response, if any.
    pending_operation: Option<OperationID>,
    /// Counter for assigning unique operation IDs **within this client**.
    next_operation: OperationID,
}

impl ClientState {
    /// Try to start the next operation. Returns `None` if a request is
    /// already in flight or no operations remain.
    fn try_next_operation(&mut self) -> Option<(OperationID, Operation)> {
        if self.pending_operation.is_some() {
            return None;
        }
        let operation = self.operations.pop_front()?;
        let operation_id = self.next_operation;
        self.next_operation += 1;
        self.pending_operation = Some(operation_id);
        Some((operation_id, operation))
    }

    /// Mark the pending operation as complete. Panics if `op_id` doesn't
    /// match the currently pending operation.
    fn complete_operation(&mut self, op_id: OperationID) {
        assert_eq!(
            self.pending_operation,
            Some(op_id),
            "response op_id {op_id} does not match pending {:?}",
            self.pending_operation
        );
        self.pending_operation = None;
    }

    fn is_done(&self) -> bool {
        self.operations.is_empty() && self.pending_operation.is_none()
    }
}

/// Discrete-event simulator that drives the key-value server and client workload.
pub struct Simulator {
    /// The server state machine.
    server: Server,
    /// Per-client workload state.
    clients: BTreeMap<ClientID, ClientState>,
    /// Priority queue of pending events, ordered by (timestamp, sequence number).
    event_queue: EventQueue,
    /// Current simulated time.
    clock: u64,
    /// Seeded RNG for reproducibility.
    rng: ChaCha8Rng,
    /// Random delay added to each message delivery.
    delivery_delay: Range<u64>,
    /// Append-only log of all events for debugging and visualization.
    action_log: Vec<LogEntry>,
    /// History of client operations for linearizability checking.
    history: History,
}

impl Simulator {
    pub fn new(server: Server, seed: u64, delivery_delay: Range<u64>) -> Self {
        Self {
            server,
            clients: BTreeMap::new(),
            event_queue: EventQueue::new(),
            clock: 0,
            rng: ChaCha8Rng::seed_from_u64(seed),
            delivery_delay,
            action_log: Vec::new(),
            history: History::new(),
        }
    }

    /// Register a client with a workload of operations to execute.
    pub fn register_client(&mut self, id: ClientID, operations: Vec<Operation>) {
        self.clients.insert(
            id,
            ClientState {
                operations: operations.into(),
                pending_operation: None,
                next_operation: 0,
            },
        );
    }

    /// Insert an event into the queue at an exact time.
    ///
    /// This is the single primitive for all event scheduling. It knows
    /// nothing about network delays — callers decide the timestamp.
    /// `send()` adds network delay before calling this; `schedule_tick_all()`
    /// passes the time directly.
    fn schedule(&mut self, event: Event, at_time: u64) {
        self.event_queue.insert(at_time, event);
    }

    /// Schedule a tick for all actors at the given time.
    pub fn schedule_tick_all(&mut self, at_time: u64) {
        self.schedule(Event::TickAll, at_time);
    }

    /// Send a message through the simulated network.
    ///
    /// Network latency is simulated by adding a random delivery delay sourced
    /// from `self.delivery_delay`.
    fn send(&mut self, message: Message) {
        let delay = (!self.delivery_delay.is_empty())
            .then(|| self.rng.random_range(self.delivery_delay.clone()))
            .unwrap_or(0);
        let deliver_at = self.clock + delay;

        self.action_log.push(LogEntry::Send {
            at: self.clock,
            deliver_at,
            message: message.clone(),
        });

        self.schedule(Event::Deliver { message }, deliver_at);
    }

    /// Send messages through the simulated network.
    fn send_all(&mut self, messages: Vec<Message>) {
        for msg in messages {
            self.send(msg);
        }
    }

    /// Start the next queued operation for a client, if possible.
    ///
    /// Returns an empty `Vec` if the client is already waiting for a
    /// response or has no operations remaining.
    fn try_next_client_operation(&mut self, client_id: ClientID) -> Vec<Message> {
        let Some(client) = self.clients.get_mut(&client_id) else {
            return vec![];
        };
        let Some((operation_id, operation)) = client.try_next_operation() else {
            return vec![];
        };
        self.history
            .record_invoke(client_id, operation_id, operation.clone(), self.clock);
        vec![Message {
            from: ActorId::Client(client_id),
            to: ActorId::Server,
            payload: MessagePayload::ClientRequest {
                operation_id,
                operation,
            },
        }]
    }

    /// Process a response delivered to a client: clear the pending operation
    /// and immediately issue the next one if available.
    ///
    /// To simulate "think time" between operations, this could instead
    /// schedule a future tick rather than dispatching immediately.
    fn process_response_to_client(
        &mut self,
        client_id: ClientID,
        message: &Message,
    ) -> Vec<Message> {
        let MessagePayload::ClientResponse {
            operation_id: op_id,
            result,
        } = &message.payload
        else {
            panic!(
                "process_response_to_client received non-ClientResponse: {:?}",
                message.payload
            );
        };
        self.history
            .record_return(client_id, *op_id, result.clone(), self.clock);
        let client = self
            .clients
            .get_mut(&client_id)
            .expect("response delivered to unregistered client");
        client.complete_operation(*op_id);
        self.try_next_client_operation(client_id)
    }

    /// Process one event in the queue. Returns `false` if the queue was empty.
    pub fn step(&mut self) -> bool {
        let Some((timestamp, event)) = self.event_queue.next() else {
            return false;
        };
        self.clock = timestamp;

        // Processing an event may produce outgoing messages, which are
        // sent through the simulated network (with delivery delay).
        let outgoing_messages = match event {
            Event::TickAll => {
                self.action_log.push(LogEntry::TickAll { at: self.clock });
                self.dispatch_tick_all()
            }
            Event::Deliver { message: msg } => {
                self.action_log.push(LogEntry::Deliver {
                    at: self.clock,
                    msg: msg.clone(),
                });
                self.dispatch_message(msg.to, &msg)
            }
        };

        self.send_all(outgoing_messages);
        true
    }

    /// Process events until the queue is empty.
    pub fn run(&mut self) {
        while self.step() {}
    }

    fn dispatch_tick_all(&mut self) -> Vec<Message> {
        let mut outgoing_messages = self.server.tick(self.clock);
        let client_ids: Vec<ClientID> = self.clients.keys().copied().collect();
        for id in client_ids {
            outgoing_messages.extend(self.try_next_client_operation(id));
        }
        outgoing_messages
    }

    fn dispatch_message(&mut self, to: ActorId, message: &Message) -> Vec<Message> {
        match to {
            ActorId::Server => self.server.on_message(message, self.clock),
            ActorId::Node(_) => vec![],
            ActorId::Client(id) => self.process_response_to_client(id, message),
        }
    }

    /// Returns `true` if every client has completed all operations.
    pub fn all_clients_done(&self) -> bool {
        self.clients.values().all(|c| c.is_done())
    }

    pub fn clock(&self) -> u64 {
        self.clock
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    /// Check whether the recorded history is linearizable.
    pub fn check_linearizable(&self) -> CheckResult {
        linearizability_checker::check_linearizable(self.history.entries())
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.action_log
    }

    /// Returns the IDs of all registered clients, in sorted order.
    pub fn client_ids(&self) -> Vec<ClientID> {
        self.clients.keys().copied().collect()
    }

    pub fn format_log(&self) -> String {
        self.action_log
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A record of something that happened during simulation.
#[derive(Debug, Clone)]
pub enum LogEntry {
    TickAll {
        at: u64,
    },
    Deliver {
        at: u64,
        msg: Message,
    },
    Send {
        at: u64,
        deliver_at: u64,
        message: Message,
    },
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogEntry::TickAll { at } => {
                write!(f, "t={at:<4} [TickAll]")
            }
            LogEntry::Deliver { at, msg } => {
                write!(
                    f,
                    "t={at:<4} [Deliver] {} -> {}: {:?}",
                    msg.from, msg.to, msg.payload,
                )
            }
            LogEntry::Send {
                at,
                deliver_at,
                message: msg,
            } => {
                write!(
                    f,
                    "t={at:<4} [Send]    {} -> {}: {:?} (deliver@{deliver_at})",
                    msg.from, msg.to, msg.payload,
                )
            }
        }
    }
}
