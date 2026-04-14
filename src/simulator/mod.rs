//! Discrete-event simulator for the key-value store.
//!
//! Owns a set of nodes, routes each client operation to one randomly-picked
//! node, and drives all state machines through a priority-queue event loop.

mod event;
mod history;
mod log;

pub use history::History;
pub use log::LogEntry;

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::ops::Range;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use tracing::warn;

use crate::analysis::history::HistoryEntry;
use crate::analysis::linearizability::{self, CheckResult};
use crate::kv::{Key, Operation, Value};
use crate::protocol::{
    ActorId, ClientID, Message, MessagePayload, NodeID, OperationID, StateMachine,
};
use crate::runtime::node::Node;

use event::{Event, EventQueue};
use log::EventLog;

/// Default number of nodes in a newly created simulator.
pub const DEFAULT_NODE_COUNT: u8 = 3;

struct ClientState {
    operations: VecDeque<Operation>,
    pending_operation: Option<OperationID>,
    next_operation: OperationID,
}

impl ClientState {
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

    fn complete_operation(&mut self, operation_id: OperationID) {
        assert_eq!(
            self.pending_operation,
            Some(operation_id),
            "response op_id {operation_id} does not match pending {:?}",
            self.pending_operation
        );
        self.pending_operation = None;
    }

    fn is_done(&self) -> bool {
        self.operations.is_empty() && self.pending_operation.is_none()
    }
}

pub struct Simulator {
    nodes: BTreeMap<NodeID, Node>,
    clients: BTreeMap<ClientID, ClientState>,
    operation_routes: BTreeMap<(ClientID, OperationID), NodeID>,
    event_queue: EventQueue,
    clock: u64,
    rng: ChaCha8Rng,
    delivery_delay: Range<u64>,
    event_log: EventLog,
    history: History,
}

impl Simulator {
    pub fn new(seed: u64, delivery_delay: Range<u64>) -> Self {
        Self::with_node_count(DEFAULT_NODE_COUNT, seed, delivery_delay)
    }

    pub fn with_node_count(node_count: u8, seed: u64, delivery_delay: Range<u64>) -> Self {
        assert!(node_count > 0, "simulator must own at least one node");
        let nodes = (0..node_count)
            .map(NodeID)
            .map(|id| (id, Node::new(id)))
            .collect();
        Self {
            nodes,
            clients: BTreeMap::new(),
            operation_routes: BTreeMap::new(),
            event_queue: EventQueue::new(),
            clock: 0,
            rng: ChaCha8Rng::seed_from_u64(seed),
            delivery_delay,
            event_log: EventLog::new(),
            history: History::new(),
        }
    }

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

    fn schedule(&mut self, event: Event, at_time: u64) {
        self.event_queue.insert(at_time, event);
    }

    pub fn schedule_tick_all(&mut self, at_time: u64) {
        self.schedule(Event::TickAll, at_time);
    }

    fn send_message(&mut self, message: Message) {
        let delay = (!self.delivery_delay.is_empty())
            .then(|| self.rng.random_range(self.delivery_delay.clone()))
            .unwrap_or(0);
        let deliver_at = self.clock + delay;

        self.event_log.record(LogEntry::Send {
            at: self.clock,
            deliver_at,
            message: message.clone(),
        });

        self.schedule(Event::Deliver { message }, deliver_at);
    }

    fn send_messages(&mut self, messages: Vec<Message>) {
        for msg in messages {
            self.send_message(msg);
        }
    }

    fn choose_node(&mut self) -> NodeID {
        let node_ids = self.node_ids();
        assert!(
            !node_ids.is_empty(),
            "simulator must have at least one node when routing an operation"
        );
        let index = self.rng.random_range(0..node_ids.len());
        node_ids[index]
    }

    fn try_next_client_operation(&mut self, client_id: ClientID) -> Option<Message> {
        let Some(client) = self.clients.get_mut(&client_id) else {
            warn!(?client_id, "simulator tried to start an operation for an unknown client");
            return None;
        };
        let Some((operation_id, operation)) = client.try_next_operation() else {
            return None;
        };
        let node_id = self.choose_node();
        assert!(
            self.operation_routes
                .insert((client_id, operation_id), node_id)
                .is_none(),
            "duplicate route recorded for {client_id} op {operation_id}"
        );
        self.history
            .record_invoke(client_id, operation_id, operation.clone(), self.clock);
        Some(Message {
            from: ActorId::Client(client_id),
            to: ActorId::Node(node_id),
            payload: MessagePayload::ClientRequest {
                operation_id,
                operation,
            },
        })
    }

    fn process_response_to_client(
        &mut self,
        client_id: ClientID,
        message: &Message,
    ) -> Vec<Message> {
        let MessagePayload::ClientResponse {
            operation_id,
            result,
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
        self.history
            .record_return(client_id, *operation_id, result.clone(), self.clock);
        let client = self
            .clients
            .get_mut(&client_id)
            .expect("response delivered to unregistered client");
        client.complete_operation(*operation_id);
        self.try_next_client_operation(client_id).into_iter().collect()
    }

    pub fn step(&mut self) -> bool {
        let Some((timestamp, event)) = self.event_queue.next() else {
            return false;
        };
        self.clock = timestamp;

        let outgoing_messages = match event {
            Event::TickAll => {
                self.event_log.record(LogEntry::TickAll { at: self.clock });
                self.dispatch_tick_all()
            }
            Event::Deliver { message: msg } => {
                self.event_log.record(LogEntry::Deliver {
                    at: self.clock,
                    msg: msg.clone(),
                });
                self.dispatch_message(msg.to, &msg)
            }
        };

        self.send_messages(outgoing_messages);
        true
    }

    pub fn run(&mut self) {
        while self.step() {}
    }

    fn dispatch_tick_all(&mut self) -> Vec<Message> {
        let client_ids: Vec<ClientID> = self.clients.keys().copied().collect();
        client_ids
            .into_iter()
            .filter_map(|id| self.try_next_client_operation(id))
            .collect()
    }

    fn dispatch_message(&mut self, to: ActorId, message: &Message) -> Vec<Message> {
        match to {
            ActorId::Node(node_id) => self.dispatch_to_node(node_id, message),
            ActorId::Client(id) => self.process_response_to_client(id, message),
        }
    }

    fn dispatch_to_node(&mut self, node_id: NodeID, message: &Message) -> Vec<Message> {
        let Some(node) = self.nodes.get_mut(&node_id) else {
            warn!(?node_id, ?message, "message delivered to an unknown node");
            return vec![];
        };
        node.on_message(message, self.clock)
    }

    pub fn all_clients_done(&self) -> bool {
        self.clients.values().all(|c| c.is_done())
    }

    pub fn is_quiescent(&self) -> bool {
        self.event_queue.is_empty() && self.all_clients_done() && self.history.all_returned()
    }

    pub fn clock(&self) -> u64 {
        self.clock
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn history_entries(&self) -> &[HistoryEntry] {
        self.history.entries()
    }

    pub fn check_linearizable(&self) -> CheckResult {
        linearizability::check_linearizable(self.history.entries())
    }

    pub fn log(&self) -> &[LogEntry] {
        self.event_log.entries()
    }

    pub fn client_ids(&self) -> Vec<ClientID> {
        self.clients.keys().copied().collect()
    }

    pub fn node_ids(&self) -> Vec<NodeID> {
        self.nodes.keys().copied().collect()
    }

    pub fn routed_node(&self, client_id: ClientID, operation_id: OperationID) -> Option<NodeID> {
        self.operation_routes.get(&(client_id, operation_id)).copied()
    }

    pub fn node_value(&self, node_id: NodeID, key: &Key) -> Option<Value> {
        self.nodes.get(&node_id).and_then(|node| node.value(key))
    }

    pub fn format_log(&self) -> String {
        self.event_log.format()
    }
}
