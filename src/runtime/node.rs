//! Database node — holds data and applies operations on that data.

use std::collections::BTreeMap;

use crate::kv::{Key, Operation, OperationResult, Value};
use crate::protocol::{ActorId, Message, MessagePayload, NodeID, StateMachine};

/// A single database node backed by an in-memory `BTreeMap`.
#[derive(Debug, Clone)]
pub struct Node {
    id: NodeID,
    database: BTreeMap<Key, Value>,
}

impl Node {
    pub fn new(id: NodeID) -> Self {
        Self {
            id,
            database: BTreeMap::new(),
        }
    }

    pub fn id(&self) -> NodeID {
        self.id
    }

    /// Return the current value for `key` without mutating state.
    pub fn value(&self, key: &Key) -> Option<Value> {
        self.database.get(key).cloned()
    }

    /// Apply an operation, mutating inner state in place.
    pub fn apply(&mut self, operation: &Operation) -> OperationResult {
        match operation {
            Operation::Put { key, value } => {
                let old_value = self.database.insert(key.clone(), value.clone());
                OperationResult(old_value)
            }
            Operation::Get { key } => {
                let current_value = self.database.get(key).cloned();
                OperationResult(current_value)
            }
            Operation::Delete { key } => {
                let old_value = self.database.remove(key);
                OperationResult(old_value)
            }
        }
    }
}

impl StateMachine for Node {
    fn on_message(&mut self, msg: &Message, _at_time: u64) -> Vec<Message> {
        let MessagePayload::ClientRequest {
            operation_id,
            ref operation,
        } = msg.payload
        else {
            return vec![];
        };
        let result = self.apply(operation);
        vec![Message {
            from: ActorId::Node(self.id),
            to: msg.from,
            payload: MessagePayload::ClientResponse {
                operation_id,
                result,
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Key {
        Key(s.into())
    }

    fn val(s: &str) -> Value {
        Value(s.into())
    }

    #[test]
    fn put_get_delete() {
        let mut node = Node::new(NodeID(0));
        assert_eq!(
            node.apply(&Operation::Get { key: key("x") }),
            OperationResult(None)
        );
        assert_eq!(
            node.apply(&Operation::Put {
                key: key("x"),
                value: val("1")
            }),
            OperationResult(None)
        );
        assert_eq!(
            node.apply(&Operation::Get { key: key("x") }),
            OperationResult(Some(val("1")))
        );
        assert_eq!(
            node.apply(&Operation::Put {
                key: key("x"),
                value: val("2")
            }),
            OperationResult(Some(val("1")))
        );
        assert_eq!(
            node.apply(&Operation::Delete { key: key("x") }),
            OperationResult(Some(val("2")))
        );
        assert_eq!(
            node.apply(&Operation::Get { key: key("x") }),
            OperationResult(None)
        );
    }
}
