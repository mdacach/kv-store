//! Event queue with deterministic ordering.
//!
//! Events are ordered by `(timestamp, sequence_number)`. The sequence number
//! deterministically breaks ties when multiple events share a timestamp,
//! ensuring FIFO order among same-time insertions — mostly for testing
//! purposes.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::Message;

/// A scheduled event in the simulation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Event {
    /// Call `tick` on every actor.
    TickAll,
    /// Deliver a message to its destination.
    Deliver { message: Message },
}

/// An event tagged with its delivery time and insertion order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TimestampedEvent {
    timestamp: u64,
    sequence_number: u64,
    event: Event,
}

impl Ord for TimestampedEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // The ordering is `reverse()`d so that the earliest event is processed
        // first.
        self.timestamp
            .cmp(&other.timestamp)
            .then_with(|| self.sequence_number.cmp(&other.sequence_number))
            .then_with(|| self.event.cmp(&other.event))
            .reverse()
    }
}

impl PartialOrd for TimestampedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) struct EventQueue {
    queue: BinaryHeap<TimestampedEvent>,
    next_sequence_number: u64,
}

impl EventQueue {
    pub(crate) fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            next_sequence_number: 0,
        }
    }

    pub(crate) fn insert(&mut self, timestamp: u64, event: Event) {
        let seq = self.next_sequence_number;
        self.next_sequence_number += 1;
        self.queue.push(TimestampedEvent {
            timestamp,
            sequence_number: seq,
            event,
        });
    }

    pub(crate) fn next(&mut self) -> Option<(u64, Event)> {
        self.queue.pop().map(|te| (te.timestamp, te.event))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
