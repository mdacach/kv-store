use std::fmt;

/// A key in the key-value store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(pub String);

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A value in the key-value store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value(pub String);

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An operation on the key-value store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    /// Insert or update a key. Returns the previous value, if any.
    Put { key: Key, value: Value },
    /// Read a key. Returns the current value.
    Get { key: Key },
    /// Remove a key. Returns the previous value.
    Delete { key: Key },
}

impl Operation {
    /// The key this operation targets.
    pub fn key(&self) -> &Key {
        match self {
            Operation::Put { key, .. } | Operation::Get { key } | Operation::Delete { key } => key,
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Put { key, value } => write!(f, "Put({key}, \"{value}\")"),
            Operation::Get { key } => write!(f, "Get({key})"),
            Operation::Delete { key } => write!(f, "Delete({key})"),
        }
    }
}

/// The result of an operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationResult(pub Option<Value>);

impl fmt::Display for OperationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(v) => write!(f, "Some(\"{v}\")"),
            None => write!(f, "None"),
        }
    }
}
