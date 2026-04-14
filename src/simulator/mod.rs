//! Discrete-event simulator for the key-value store.
//!
//! Owns a set of nodes, routes each client request to one randomly-picked node,
//! and drives all state machines through a priority-queue event loop.

mod event;
mod history;
mod log;

pub use history::RequestHistory;
pub use log::{EventEntry, EventLog};

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::ops::Range;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use tracing::warn;

use crate::analysis::history::HistoryEntry;
use crate::analysis::linearizability::{self, CheckResult};
use crate::kv::{Key, Request, Value};
use crate::protocol::{
    ActorId, ClientID, Message, MessagePayload, NodeID, RequestID, StateMachine,
};
use crate::runtime::node::Node;

use event::{Event, EventQueue};

/// Default number of nodes in a newly created simulator.
pub const DEFAULT_NODE_COUNT: u8 = 3;

/// Per-client workload state, managed by the simulator.
///
/// Clients use **stop-and-wait**: a client sends one request, waits for
/// its response, and only then sends the next.
struct ClientState {
    /// Requests remaining to be sent, in order.
    requests: VecDeque<Request>,
    /// The request currently awaiting a response, if any.
    pending_request: Option<RequestID>,
    /// Counter for assigning unique request IDs **within this client**.
    next_request: RequestID,
}

impl ClientState {
    /// Try to start the next request. Returns `None` if a request is
    /// already in flight or no requests remain.
    fn try_next_request(&mut self) -> Option<(RequestID, Request)> {
        if self.pending_request.is_some() {
            return None;
        }
        let request = self.requests.pop_front()?;
        let request_id = self.next_request;
        self.next_request += 1;
        self.pending_request = Some(request_id);
        Some((request_id, request))
    }

    /// Mark the pending request as complete. Panics if `request_id` doesn't
    /// match the currently pending request.
    fn complete_request(&mut self, request_id: RequestID) {
        assert_eq!(
            self.pending_request,
            Some(request_id),
            "response request_id {request_id} does not match pending {:?}",
            self.pending_request
        );
        self.pending_request = None;
    }

    fn is_done(&self) -> bool {
        self.requests.is_empty() && self.pending_request.is_none()
    }
}

/// Discrete-event simulator that drives the node set and client workload.
pub struct Simulator {
    /// Concrete node state machines addressed by `ActorId::Node(_)`.
    nodes: BTreeMap<NodeID, Node>,
    /// Per-client workload state.
    clients: BTreeMap<ClientID, ClientState>,
    /// Chosen target node for each completed or pending request.
    request_routes: BTreeMap<(ClientID, RequestID), NodeID>,
    /// Priority queue of pending events, ordered by (timestamp, sequence number).
    event_queue: EventQueue,
    /// Current simulated time.
    clock: u64,
    /// Seeded RNG for reproducibility.
    rng: ChaCha8Rng,
    /// Random delay added to each message delivery.
    delivery_delay: Range<u64>,
    /// Append-only log of simulator events for debugging and visualization.
    ///
    /// This log keeps every send, delivery, and tick, so it is more detailed
    /// than [`request_history`](Self::request_history), which keeps only one
    /// completed entry per client request for linearizability checking.
    event_log: EventLog,
    /// Completed request intervals used by linearizability analysis.
    ///
    /// Unlike [`event_log`](Self::event_log), this tracks only request/response
    /// pairs with invocation and return times. That smaller view is what the
    /// correctness checker needs.
    request_history: RequestHistory,
}

impl Simulator {
    /// Creates a simulator with [`DEFAULT_NODE_COUNT`] nodes.
    pub fn new(seed: u64, delivery_delay: Range<u64>) -> Self {
        Self::with_node_count(DEFAULT_NODE_COUNT, seed, delivery_delay)
    }

    /// Creates a simulator with an explicit node count and delivery-delay range.
    ///
    /// `seed` controls all randomized choices, including request routing and
    /// message delays, so repeated runs with the same inputs stay deterministic.
    pub fn with_node_count(node_count: u8, seed: u64, delivery_delay: Range<u64>) -> Self {
        assert!(node_count > 0, "simulator must own at least one node");
        let nodes = (0..node_count)
            .map(NodeID)
            .map(|id| (id, Node::new(id)))
            .collect();
        Self {
            nodes,
            clients: BTreeMap::new(),
            request_routes: BTreeMap::new(),
            event_queue: EventQueue::new(),
            clock: 0,
            rng: ChaCha8Rng::seed_from_u64(seed),
            delivery_delay,
            event_log: EventLog::new(),
            request_history: RequestHistory::new(),
        }
    }

    /// Registers a client with a stop-and-wait workload of requests.
    ///
    /// The workload is executed in order. The client will not issue request
    /// `n + 1` until request `n` has received a response.
    pub fn register_client(&mut self, id: ClientID, requests: Vec<Request>) {
        self.clients.insert(
            id,
            ClientState {
                requests: requests.into(),
                pending_request: None,
                next_request: 0,
            },
        );
    }

    /// Inserts an event into the queue at an exact simulated time.
    ///
    /// The queue itself does not model latency. Simulated delay is introduced
    /// by callers choosing an appropriate `at_time` for delivery.
    fn schedule(&mut self, event: Event, at_time: u64) {
        self.event_queue.insert(at_time, event);
    }

    /// Schedules a global tick at `at_time`.
    ///
    /// This convenience wrapper keeps tests and examples readable when they
    /// want to kick off one simulator round explicitly.
    pub fn schedule_tick_all(&mut self, at_time: u64) {
        self.schedule(Event::TickAll, at_time);
    }

    /// Send a message through the simulated network.
    ///
    /// Network latency is simulated by adding a random delivery delay sourced
    /// from `self.delivery_delay`.
    fn send_message(&mut self, message: Message) {
        let delay = (!self.delivery_delay.is_empty())
            .then(|| self.rng.random_range(self.delivery_delay.clone()))
            .unwrap_or(0);
        let deliver_at = self.clock + delay;

        self.event_log.record(EventEntry::Send {
            at: self.clock,
            deliver_at,
            message: message.clone(),
        });

        self.schedule(Event::Deliver { message }, deliver_at);
    }

    /// Send messages through the simulated network.
    fn send_messages(&mut self, messages: Vec<Message>) {
        for msg in messages {
            self.send_message(msg);
        }
    }

    /// Chooses one node to handle the next client request.
    ///
    /// Selection uses the simulator's seeded RNG, so routing stays reproducible
    /// for a fixed seed.
    fn choose_node(&mut self) -> NodeID {
        let node_ids = self.node_ids();
        assert!(
            !node_ids.is_empty(),
            "simulator must have at least one node when routing a request"
        );
        let index = self.rng.random_range(0..node_ids.len());
        node_ids[index]
    }

    /// Start the next queued request for a client, if possible.
    ///
    /// Returns `None` if the client is already waiting for a response or has
    /// no requests remaining.
    fn try_next_client_request(&mut self, client_id: ClientID) -> Option<Message> {
        let Some(client) = self.clients.get_mut(&client_id) else {
            warn!(?client_id, "simulator tried to start a request for an unknown client");
            return None;
        };
        let Some((request_id, request)) = client.try_next_request() else {
            return None;
        };
        let node_id = self.choose_node();
        assert!(
            self.request_routes
                .insert((client_id, request_id), node_id)
                .is_none(),
            "duplicate route recorded for {client_id} request {request_id}"
        );
        self.request_history
            .record_request(client_id, request_id, request.clone(), self.clock);
        Some(Message {
            from: ActorId::Client(client_id),
            to: ActorId::Node(node_id),
            payload: MessagePayload::ClientRequest {
                request_id,
                request,
            },
        })
    }

    /// Process a response delivered to a client: clear the pending request
    /// and immediately issue the next one if available.
    ///
    /// This models zero think time between requests. Changing the simulator to
    /// wait before issuing the next request would not break the core
    /// correctness machinery, but it would change the recorded timings and
    /// require scheduling a future event instead of dispatching immediately.
    fn process_response_to_client(
        &mut self,
        client_id: ClientID,
        message: &Message,
    ) -> Vec<Message> {
        let MessagePayload::ClientResponse {
            request_id,
            response,
        } = &message.payload
        else {
            panic!(
                "process_response_to_client received non-ClientResponse: {:?}",
                message.payload
            );
        };
        assert_eq!(
            message.to,
            ActorId::Client(client_id),
            "response recipient must match the client being completed"
        );
        self.request_history
            .record_response(client_id, *request_id, response.clone(), self.clock);
        let client = self
            .clients
            .get_mut(&client_id)
            .expect("response delivered to unregistered client");
        client.complete_request(*request_id);
        self.try_next_client_request(client_id).into_iter().collect()
    }

    /// Process one event in the queue. Returns `false` if the queue was empty.
    pub fn step(&mut self) -> bool {
        let Some((timestamp, event)) = self.event_queue.next() else {
            return false;
        };
        self.clock = timestamp;

        let outgoing_messages = match event {
            Event::TickAll => {
                self.event_log.record(EventEntry::TickAll { at: self.clock });
                self.dispatch_tick_all()
            }
            Event::Deliver { message: msg } => {
                self.event_log.record(EventEntry::Deliver {
                    at: self.clock,
                    msg: msg.clone(),
                });
                self.dispatch_message(msg.to, &msg)
            }
        };

        self.send_messages(outgoing_messages);
        true
    }

    /// Process events until the queue is empty.
    pub fn run(&mut self) {
        while self.step() {}
    }

    /// Asks every registered client to try issuing its next request.
    ///
    /// Because clients use stop-and-wait, each client contributes at most one
    /// outbound request per tick.
    fn dispatch_tick_all(&mut self) -> Vec<Message> {
        let client_ids: Vec<ClientID> = self.clients.keys().copied().collect();
        client_ids
            .into_iter()
            .filter_map(|id| self.try_next_client_request(id))
            .collect()
    }

    /// Dispatches one delivered message to its destination actor.
    fn dispatch_message(&mut self, to: ActorId, message: &Message) -> Vec<Message> {
        match to {
            ActorId::Node(node_id) => self.dispatch_to_node(node_id, message),
            ActorId::Client(id) => self.process_response_to_client(id, message),
        }
    }

    /// Delivers one message to a node state machine.
    fn dispatch_to_node(&mut self, node_id: NodeID, message: &Message) -> Vec<Message> {
        let Some(node) = self.nodes.get_mut(&node_id) else {
            warn!(?node_id, ?message, "message delivered to an unknown node");
            return vec![];
        };
        node.on_message(message, self.clock)
    }

    /// Returns `true` if every client has completed its workload.
    ///
    /// This does not imply quiescence on its own: messages can still be queued
    /// in transit. Use [`is_quiescent`](Self::is_quiescent) when callers need
    /// both workload completion and an empty event queue.
    pub fn all_clients_done(&self) -> bool {
        self.clients.values().all(|c| c.is_done())
    }

    /// Returns `true` when the simulator has no more work in flight.
    pub fn is_quiescent(&self) -> bool {
        self.event_queue.is_empty()
            && self.all_clients_done()
            && self.request_history.all_responded()
    }

    /// Returns the current simulated time.
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Returns the completed request history used for correctness checks.
    pub fn request_history(&self) -> &RequestHistory {
        &self.request_history
    }

    /// Returns the completed request entries in completion order.
    pub fn request_entries(&self) -> &[HistoryEntry] {
        self.request_history.entries()
    }

    /// Check whether the recorded history is linearizable.
    pub fn check_linearizable(&self) -> CheckResult {
        linearizability::check_linearizable(self.request_history.entries())
    }

    /// Returns the detailed simulator event log used by trace visualizations.
    pub fn event_log(&self) -> &EventLog {
        &self.event_log
    }

    /// Returns the IDs of all registered clients, in sorted order.
    pub fn client_ids(&self) -> Vec<ClientID> {
        self.clients.keys().copied().collect()
    }

    /// Returns the IDs of all simulated nodes, in sorted order.
    pub fn node_ids(&self) -> Vec<NodeID> {
        self.nodes.keys().copied().collect()
    }

    /// Returns the chosen node for a routed client request, if known.
    ///
    /// This is useful in tests and visualizations that need to explain why one
    /// request observed different state from another.
    pub fn routed_node(&self, client_id: ClientID, request_id: RequestID) -> Option<NodeID> {
        self.request_routes.get(&(client_id, request_id)).copied()
    }

    /// Returns the current value stored for `key` on one specific node.
    pub fn node_value(&self, node_id: NodeID, key: &Key) -> Option<Value> {
        self.nodes.get(&node_id).and_then(|node| node.value(key))
    }

    /// Formats the event log as plain text for debugging and test failures.
    pub fn format_log(&self) -> String {
        self.event_log.format()
    }
}
