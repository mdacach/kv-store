pub mod node;
pub mod server;
pub mod simulator;

use std::fmt;

pub use node::{Key, Node, Operation, OperationResult, Value};
pub use server::Server;

/// Unique identifier for a client operation **within a single client**.
pub type OperationID = u64;

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
    // The server, responsible for routing requests to nodes.
    Server,
    // Nodes maintain internal databases and serve requests.
    Node(NodeID),
    // Simulated clients that issue requests to the server.
    Client(ClientID),
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActorId::Server => write!(f, "Server"),
            ActorId::Node(id) => write!(f, "Node({id})"),
            ActorId::Client(id) => write!(f, "Client({id})"),
        }
    }
}

/// Payload of a message exchanged between actors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePayload {
    /// Client -> Server: request to execute an operation.
    ClientRequest {
        operation_id: OperationID,
        operation: Operation,
    },
    /// Server -> Client: result of a completed operation.
    ClientResponse {
        operation_id: OperationID,
        result: OperationResult,
    },
}

/// A message in transit between two actors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Message {
    pub from: ActorId,
    pub to: ActorId,
    pub payload: MessagePayload,
}

/// A protocol actor driven by the simulator.
pub trait StateMachine {
    /// Handle an inbound message and return any messages to send in response.
    fn on_message(&mut self, msg: &Message, at_time: u64) -> Vec<Message>;

    /// Simulate passage of time. May produce spontaneous messages, such as new requests or heartbeats.
    fn tick(&mut self, _at_time: u64) -> Vec<Message> {
        vec![]
    }
}
