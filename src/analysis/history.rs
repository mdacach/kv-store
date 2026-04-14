use crate::kv::{Request, Response};
use crate::protocol::ClientID;

/// A completed client request with timing information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// The client that issued the request.
    pub client_id: ClientID,
    /// The request that was executed.
    pub request: Request,
    /// The simulated time when the client sent the request.
    pub invoke_time: u64,
    /// The simulated time when the client received the response.
    pub return_time: u64,
    /// The response observed by the client.
    pub response: Response,
}
