//! Server — client-facing routing layer.
//!
//! Receives `ClientRequest` messages and routes them to the appropriate node.

use crate::node::Node;
use crate::{Message, StateMachine};

pub struct Server {
    node: Node,
}

impl Server {
    pub fn new(node: Node) -> Self {
        Self { node }
    }
}

impl StateMachine for Server {
    fn on_message(&mut self, message: &Message, at_time: u64) -> Vec<Message> {
        self.node.on_message(message, at_time)
    }
}
