//! Event-log data structures for simulator execution traces.
//!
//! The event log is append-only and records every externally visible simulator
//! step in timestamp order. Unlike [`RequestHistory`](super::RequestHistory),
//! which keeps one entry per completed client request for correctness checks,
//! the event log keeps all send, delivery, and tick events for debugging and
//! visualization.

use std::fmt;

use crate::Message;

/// An append-only event log for simulator execution.
#[derive(Debug, Clone, Default)]
pub struct EventLog {
    entries: Vec<EventEntry>,
}

impl EventLog {
    /// Creates an empty event log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one event to the log.
    pub fn record(&mut self, entry: EventEntry) {
        self.entries.push(entry);
    }

    /// Returns the recorded events in insertion order.
    pub fn entries(&self) -> &[EventEntry] {
        &self.entries
    }

    /// Returns `true` when no events have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Formats the event log as one human-readable line per event.
    pub fn format(&self) -> String {
        self.entries
            .iter()
            .map(|entry| entry.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One entry in the simulator event log.
#[derive(Debug, Clone)]
pub enum EventEntry {
    /// Records that the simulator asked every actor to tick at `at`.
    TickAll { at: u64 },
    /// Records that a queued message reached its destination at `at`.
    Deliver { at: u64, msg: Message },
    /// Records that a message was sent at `at` and is scheduled to arrive at
    /// `deliver_at`.
    ///
    /// `Send` stores both timestamps because sends are the only events that
    /// span an interval in simulated time. Visualizations use that pair to
    /// draw an in-flight message rather than a single point event.
    Send {
        at: u64,
        deliver_at: u64,
        message: Message,
    },
}

impl fmt::Display for EventEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventEntry::TickAll { at } => {
                write!(f, "t={at:<4} [TickAll]")
            }
            EventEntry::Deliver { at, msg } => {
                write!(
                    f,
                    "t={at:<4} [Deliver] {} -> {}: {:?}",
                    msg.from, msg.to, msg.payload,
                )
            }
            EventEntry::Send {
                at,
                deliver_at,
                message: msg,
            } => {
                write!(
                    f,
                    "t={at:<4} [Send]    {} -> {}: {:?} (deliver@{deliver_at})",
                    msg.from, msg.to, msg.payload,
                )
            }
        }
    }
}
