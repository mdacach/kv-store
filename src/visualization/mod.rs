//! Visualization modules for the KV store simulation.
//!
//! - [`linearizability`] — swim-lane diagram showing requests as time intervals
//!   with linearization point markers and reference state transitions.
//!   Answers: "is this history linearizable, and if not, where does it break?"
//!
//! - [`trace`] — message-flow diagram showing Send/Deliver arrows between actors
//!   on a timeline with step-by-step playback.
//!   Answers: "what messages were exchanged and when?"

pub mod linearizability;
pub mod trace;
