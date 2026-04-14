//! Database node — holds data and serves requests against that data.

use std::collections::BTreeMap;

use crate::kv::{Key, Request, Response, Value};
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

    /// Apply a request, mutating inner state in place.
    pub fn apply(&mut self, request: &Request) -> Response {
        match request {
            Request::Put { key, value } => {
                let old_value = self.database.insert(key.clone(), value.clone());
                Response(old_value)
            }
            Request::Get { key } => {
                let current_value = self.database.get(key).cloned();
                Response(current_value)
            }
            Request::Delete { key } => {
                let old_value = self.database.remove(key);
                Response(old_value)
            }
        }
    }
}

impl StateMachine for Node {
    fn on_message(&mut self, msg: &Message, _at_time: u64) -> Vec<Message> {
        let MessagePayload::ClientRequest {
            request_id,
            ref request,
        } = msg.payload
        else {
            return vec![];
        };
        let response = self.apply(request);
        vec![Message {
            from: ActorId::Node(self.id),
            to: msg.from,
            payload: MessagePayload::ClientResponse {
                request_id,
                response,
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
            node.apply(&Request::Get { key: key("x") }),
            Response(None)
        );
        assert_eq!(
            node.apply(&Request::Put {
                key: key("x"),
                value: val("1")
            }),
            Response(None)
        );
        assert_eq!(
            node.apply(&Request::Get { key: key("x") }),
            Response(Some(val("1")))
        );
        assert_eq!(
            node.apply(&Request::Put {
                key: key("x"),
                value: val("2")
            }),
            Response(Some(val("1")))
        );
        assert_eq!(
            node.apply(&Request::Delete { key: key("x") }),
            Response(Some(val("2")))
        );
        assert_eq!(
            node.apply(&Request::Get { key: key("x") }),
            Response(None)
        );
    }
}
