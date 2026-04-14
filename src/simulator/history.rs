//! Request tracking for simulated clients.
//!
//! Captures every client request/response pair with timestamps, producing the
//! data needed by analysis: for each request, the invocation time (when the
//! client sent the request) and the return time (when the client received the
//! response).

use std::collections::BTreeMap;

use crate::analysis::history::HistoryEntry;
use crate::kv::{Request, Response};
use crate::protocol::{ClientID, RequestID};

/// In-flight request awaiting a response.
#[derive(Debug, Clone)]
struct PendingEntry {
    request: Request,
    invoke_time: u64,
}

/// Records completed client requests for linearizability checking.
///
/// The simulator calls `record_request` when a client sends a request and
/// `record_response` when the response arrives. Completed requests are
/// stored in insertion order for later analysis.
#[derive(Debug, Default)]
pub struct RequestHistory {
    completed: Vec<HistoryEntry>,
    pending: BTreeMap<(ClientID, RequestID), PendingEntry>,
}

impl RequestHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a client has issued a request at the given time.
    pub fn record_request(
        &mut self,
        client_id: ClientID,
        request_id: RequestID,
        request: Request,
        at_time: u64,
    ) {
        let prev = self.pending.insert(
            (client_id, request_id),
            PendingEntry {
                request,
                invoke_time: at_time,
            },
        );
        assert!(
            prev.is_none(),
            "duplicate request for {client_id} request {request_id}"
        );
    }

    /// Record that a client has received a response at the given time.
    ///
    /// Panics if there is no matching pending request.
    pub fn record_response(
        &mut self,
        client_id: ClientID,
        request_id: RequestID,
        response: Response,
        at_time: u64,
    ) {
        let entry = self
            .pending
            .remove(&(client_id, request_id))
            .unwrap_or_else(|| {
                panic!("response without request for {client_id} request {request_id}")
            });
        self.completed.push(HistoryEntry {
            client_id,
            request: entry.request,
            invoke_time: entry.invoke_time,
            return_time: at_time,
            response,
        });
    }

    /// All completed requests, in order of completion.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.completed
    }

    /// Returns true if there are no pending (in-flight) requests.
    pub fn all_responded(&self) -> bool {
        self.pending.is_empty()
    }
}
