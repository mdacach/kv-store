//! Request and response data types for the key-value store model.
//!
//! Keys and values are modeled as owned strings. That is enough for this
//! project because the simulator focuses on routing, timing, and consistency
//! behavior rather than binary encodings or type-rich application payloads.

use std::fmt;

/// A string key in the modeled key-value store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(pub String);

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A string value in the modeled key-value store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value(pub String);

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A client request against the key-value store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Request {
    /// Insert or update a key. Returns the previous value, if any.
    Put { key: Key, value: Value },
    /// Read a key. Returns the current value.
    Get { key: Key },
    /// Remove a key. Returns the previous value.
    Delete { key: Key },
}

impl Request {
    /// The key this request targets.
    pub fn key(&self) -> &Key {
        match self {
            Request::Put { key, .. } | Request::Get { key } | Request::Delete { key } => key,
        }
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Request::Put { key, value } => write!(f, "Put({key}, \"{value}\")"),
            Request::Get { key } => write!(f, "Get({key})"),
            Request::Delete { key } => write!(f, "Delete({key})"),
        }
    }
}

/// A response to a client request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Response(pub Option<Value>);

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(v) => write!(f, "Some(\"{v}\")"),
            None => write!(f, "None"),
        }
    }
}
