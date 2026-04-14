//! Tools for simulating, analyzing, and visualizing a replicated key-value
//! store.
//!
//! The crate is organized around four main concerns:
//!
//! - [`kv`] defines the request/response data model used throughout the crate.
//! - [`runtime`] contains executable state machines such as [`Node`].
//! - [`simulator`] drives nodes and clients through a deterministic
//!   discrete-event loop while recording request history and event traces.
//! - [`analysis`] checks the recorded request history for properties such as
//!   linearizability, and [`visualization`] renders those traces for humans.
//!
//! # Example
//!
//! ```rust
//! use kv_store::{ClientID, Request, Simulator, Value, Key};
//!
//! let mut sim = Simulator::new(7, 1..3);
//! sim.register_client(
//!     ClientID(0),
//!     vec![
//!         Request::Put {
//!             key: Key("x".into()),
//!             value: Value("1".into()),
//!         },
//!         Request::Get {
//!             key: Key("x".into()),
//!         },
//!     ],
//! );
//! sim.schedule_tick_all(0);
//! sim.run();
//!
//! assert!(sim.request_history().all_responded());
//! ```

pub mod analysis;
pub mod kv;
pub mod protocol;
pub mod runtime;
pub mod simulator;
pub mod visualization;

pub use analysis::history::HistoryEntry;
pub use analysis::linearizability::{check_linearizable, CheckResult};
pub use kv::{Key, Request, Response, Value};
pub use protocol::{ActorId, ClientID, Message, MessagePayload, NodeID, RequestID, StateMachine};
pub use runtime::node::Node;
pub use simulator::{EventEntry, EventLog, RequestHistory, Simulator, DEFAULT_NODE_COUNT};
