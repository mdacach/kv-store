//! History recording for client operations.
//!
//! Captures every client request/response pair with timestamps, producing the
//! data needed by a linearizability checker: for each operation, the invocation
//! time (when the client sent the request) and the return time (when the client
//! received the response).

use std::collections::BTreeMap;

use crate::node::{Operation, OperationResult};
use crate::{ClientID, OperationID};

/// A completed client operation with timing information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub client_id: ClientID,
    pub operation: Operation,
    pub invoke_time: u64,
    pub return_time: u64,
    pub result: OperationResult,
}

/// In-flight operation awaiting a response.
#[derive(Debug, Clone)]
struct PendingEntry {
    operation: Operation,
    invoke_time: u64,
}

/// Records client operations as they flow through the simulation.
///
/// The simulator calls `record_invoke` when a client sends a request and
/// `record_return` when the response arrives. Completed operations are
/// stored in insertion order for later analysis.
#[derive(Debug, Default)]
pub struct History {
    completed: Vec<HistoryEntry>,
    pending: BTreeMap<(ClientID, OperationID), PendingEntry>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a client has invoked an operation at the given time.
    pub fn record_invoke(
        &mut self,
        client_id: ClientID,
        operation_id: OperationID,
        operation: Operation,
        at_time: u64,
    ) {
        let prev = self.pending.insert(
            (client_id, operation_id),
            PendingEntry {
                operation,
                invoke_time: at_time,
            },
        );
        assert!(
            prev.is_none(),
            "duplicate invoke for {client_id} op {operation_id}"
        );
    }

    /// Record that a client has received a response at the given time.
    ///
    /// Panics if there is no matching pending invocation.
    pub fn record_return(
        &mut self,
        client_id: ClientID,
        operation_id: OperationID,
        result: OperationResult,
        at_time: u64,
    ) {
        let entry = self
            .pending
            .remove(&(client_id, operation_id))
            .unwrap_or_else(|| {
                panic!("return without invoke for {client_id} op {operation_id}")
            });
        self.completed.push(HistoryEntry {
            client_id,
            operation: entry.operation,
            invoke_time: entry.invoke_time,
            return_time: at_time,
            result,
        });
    }

    /// All completed operations, in order of completion.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.completed
    }

    /// Returns true if there are no pending (in-flight) operations.
    pub fn all_returned(&self) -> bool {
        self.pending.is_empty()
    }
}
