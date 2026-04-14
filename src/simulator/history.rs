//! History recording for client operations.
//!
//! Captures every client request/response pair with timestamps, producing the
//! data needed by analysis: for each operation, the invocation time (when the
//! client sent the request) and the return time (when the client received the
//! response).

use std::collections::BTreeMap;

use crate::analysis::history::HistoryEntry;
use crate::kv::{Operation, OperationResult};
use crate::protocol::{ClientID, OperationID};

/// In-flight operation awaiting a response.
#[derive(Debug, Clone)]
struct PendingEntry {
    operation: Operation,
    invoke_time: u64,
}

/// Records client operations as they flow through the simulation.
#[derive(Debug, Default)]
pub struct History {
    completed: Vec<HistoryEntry>,
    pending: BTreeMap<(ClientID, OperationID), PendingEntry>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

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
            .unwrap_or_else(|| panic!("return without invoke for {client_id} op {operation_id}"));
        self.completed.push(HistoryEntry {
            client_id,
            operation: entry.operation,
            invoke_time: entry.invoke_time,
            return_time: at_time,
            result,
        });
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.completed
    }

    pub fn all_returned(&self) -> bool {
        self.pending.is_empty()
    }
}
