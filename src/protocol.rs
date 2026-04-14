//! Protocol-level identifiers and message types shared by the runtime and simulator.

use std::fmt;

/// Unique identifier for a client request **within a single client**.
pub type RequestID = u64;

/// Identifier for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeID(pub u8);

impl fmt::Display for NodeID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.0)
    }
}

/// Identifier for a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientID(pub u8);

impl fmt::Display for ClientID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client({})", self.0)
    }
}

/// Identifier for any actor in the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActorId {
    // Nodes maintain internal databases and serve requests.
    Node(NodeID),
    // Simulated clients that issue requests to nodes.
    Client(ClientID),
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActorId::Node(id) => write!(f, "Node({id})"),
            ActorId::Client(id) => write!(f, "Client({id})"),
        }
    }
}

/// Payload of a message exchanged between actors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePayload {
    /// Request to execute a client request.
    ClientRequest {
        request_id: RequestID,
        request: crate::kv::Request,
    },
    /// Response for a completed request.
    ClientResponse {
        request_id: RequestID,
        response: crate::kv::Response,
    },
}

/// A message in transit between two actors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Message {
    /// The actor that sent the message.
    pub from: ActorId,
    /// The actor that should receive the message.
    pub to: ActorId,
    /// The message payload.
    pub payload: MessagePayload,
}

/// A protocol actor driven by the simulator.
pub trait StateMachine {
    /// Handle an inbound message and return any messages to send in response.
    fn on_message(&mut self, message: &Message, at_time: u64) -> Vec<Message>;

    /// Simulate passage of time. May produce spontaneous messages, such as new requests or heartbeats.
    fn tick(&mut self, _at_time: u64) -> Vec<Message> {
        vec![]
    }
}
