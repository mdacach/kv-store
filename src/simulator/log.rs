use std::fmt;

use crate::Message;

#[derive(Debug, Clone, Default)]
pub(crate) struct EventLog {
    entries: Vec<LogEntry>,
}

impl EventLog {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn record(&mut self, entry: LogEntry) {
        self.entries.push(entry);
    }

    pub(crate) fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub(crate) fn format(&self) -> String {
        self.entries
            .iter()
            .map(|entry| entry.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A record of something that happened during simulation.
#[derive(Debug, Clone)]
pub enum LogEntry {
    TickAll {
        at: u64,
    },
    Deliver {
        at: u64,
        msg: Message,
    },
    Send {
        at: u64,
        deliver_at: u64,
        message: Message,
    },
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogEntry::TickAll { at } => {
                write!(f, "t={at:<4} [TickAll]")
            }
            LogEntry::Deliver { at, msg } => {
                write!(
                    f,
                    "t={at:<4} [Deliver] {} -> {}: {:?}",
                    msg.from, msg.to, msg.payload,
                )
            }
            LogEntry::Send {
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
