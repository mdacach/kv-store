//! Brute-force linearizability checker.
//!
//! Determines whether a concurrent history of key-value operations is linearizable
//! by searching for a valid sequential ordering via recursive backtracking.
//!
//! The sequential reference is a standalone `BTreeMap<Key, Value>`, that keeps
//! track of current values for each key.

use std::collections::BTreeMap;

use crate::history::HistoryEntry;
use crate::node::{Key, Operation, OperationResult, Value};

/// Result of a linearizability check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    /// A valid linearization exists.
    Ok,
    /// No valid linearization exists.
    Violation,
}

/// Apply an operation to the reference.
fn apply_to_reference(state: &mut BTreeMap<Key, Value>, operation: &Operation) -> OperationResult {
    match operation {
        Operation::Put { key, value } => OperationResult(state.insert(key.clone(), value.clone())),
        Operation::Get { key } => OperationResult(state.get(key).cloned()),
        Operation::Delete { key } => OperationResult(state.remove(key)),
    }
}

/// Check whether a history is linearizable.
///
/// Tries all valid orderings of concurrent operations via backtracking.
/// An operation is "eligible" to be linearized next only if no un-linearized
/// operation happens-before it (i.e., has a strictly earlier return_time
/// than this operation's invoke_time).
pub fn check_linearizable(entries: &[HistoryEntry]) -> CheckResult {
    let mut linearized = vec![false; entries.len()];
    let state = BTreeMap::new();
    try_linearize(entries, &mut linearized, state)
}

fn try_linearize(
    entries: &[HistoryEntry],
    linearized: &mut Vec<bool>,
    state: BTreeMap<Key, Value>,
) -> CheckResult {
    // If all entries have been linearized, we're done.
    if linearized.iter().all(|&l| l) {
        return CheckResult::Ok;
    }

    // Otherwise, let's pick an un-linearized entry to process next.
    for i in 0..entries.len() {
        if linearized[i] {
            continue;
        }

        // This entry is already a valid candidate in theory, because it hasn't
        // been linearized yet. But it's possible that _another_ entry
        // happens-before this one and checking _that_ one first would be
        // better.
        let other_happens_before = entries.iter().enumerate().any(|(j, other)| {
            !linearized[j] && j != i && other.return_time < entries[i].invoke_time
        });
        // Because there must exist such "earliest" candidate entry, we're
        // guaranteed to process it sometime...
        if other_happens_before {
            continue;
        }

        // Try linearizing this operation at this position.
        let mut new_state = state.clone();
        let result = apply_to_reference(&mut new_state, &entries[i].operation);
        // If results agree between the reference model and our implementation,
        // we're free to continue checking.
        if result == entries[i].result {
            linearized[i] = true;
            if try_linearize(entries, linearized, new_state) == CheckResult::Ok {
                // Nice!
                return CheckResult::Ok;
            }
            // Otherwise, this wasn't a correct pick. Marking this as not linearized
            // will make the previous iteration choose another one next.
            linearized[i] = false;
        }
    }

    CheckResult::Violation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClientID;

    fn key(s: &str) -> Key {
        Key(s.into())
    }

    fn val(s: &str) -> Value {
        Value(s.into())
    }

    fn entry(
        client: u8,
        op: Operation,
        invoke: u64,
        ret: u64,
        result: Option<Value>,
    ) -> HistoryEntry {
        HistoryEntry {
            client_id: ClientID(client),
            operation: op,
            invoke_time: invoke,
            return_time: ret,
            result: OperationResult(result),
        }
    }

    mod linearizable {
        use super::*;

        #[test]
        fn empty_history() {
            assert_eq!(check_linearizable(&[]), CheckResult::Ok);
        }

        #[test]
        fn sequential_put_then_get() {
            let history = vec![
                entry(
                    0,
                    Operation::Put {
                        key: key("x"),
                        value: val("1"),
                    },
                    0,
                    1,
                    None,
                ),
                entry(0, Operation::Get { key: key("x") }, 2, 3, Some(val("1"))),
            ];
            assert_eq!(check_linearizable(&history), CheckResult::Ok);
        }

        #[test]
        fn concurrent_puts() {
            // Two concurrent puts on the same key. In the linearization
            // Put("1") goes first (returns None), then Put("2") (returns Some("1")).
            // The subsequent get sees "2".
            let history = vec![
                entry(
                    0,
                    Operation::Put {
                        key: key("x"),
                        value: val("1"),
                    },
                    0,
                    5,
                    None,
                ),
                entry(
                    1,
                    Operation::Put {
                        key: key("x"),
                        value: val("2"),
                    },
                    0,
                    5,
                    Some(val("1")),
                ),
                entry(0, Operation::Get { key: key("x") }, 6, 7, Some(val("2"))),
            ];
            assert_eq!(check_linearizable(&history), CheckResult::Ok);
        }

        #[test]
        fn concurrent_ops_on_different_keys() {
            let history = vec![
                entry(
                    0,
                    Operation::Put {
                        key: key("x"),
                        value: val("1"),
                    },
                    0,
                    3,
                    None,
                ),
                entry(
                    1,
                    Operation::Put {
                        key: key("y"),
                        value: val("2"),
                    },
                    1,
                    4,
                    None,
                ),
                entry(0, Operation::Get { key: key("x") }, 5, 6, Some(val("1"))),
                entry(1, Operation::Get { key: key("y") }, 5, 6, Some(val("2"))),
            ];
            assert_eq!(check_linearizable(&history), CheckResult::Ok);
        }

        #[test]
        fn put_get_delete_get_sequential() {
            let history = vec![
                entry(
                    0,
                    Operation::Put {
                        key: key("x"),
                        value: val("1"),
                    },
                    0,
                    1,
                    None,
                ),
                entry(0, Operation::Get { key: key("x") }, 2, 3, Some(val("1"))),
                entry(0, Operation::Delete { key: key("x") }, 4, 5, Some(val("1"))),
                entry(0, Operation::Get { key: key("x") }, 6, 7, None),
            ];
            assert_eq!(check_linearizable(&history), CheckResult::Ok);
        }
    }

    mod violations {
        use super::*;

        #[test]
        fn stale_read() {
            // Client 0 writes Put(x, "1") and it returns.
            // The subsequent Get(x) sees None — but the put
            // happens-before the get, so it must see "1".
            let history = vec![
                entry(
                    0,
                    Operation::Put {
                        key: key("x"),
                        value: val("1"),
                    },
                    0,
                    1,
                    None,
                ),
                entry(0, Operation::Get { key: key("x") }, 2, 3, None),
            ];
            assert_eq!(check_linearizable(&history), CheckResult::Violation);
        }

        #[test]
        fn concurrent_ops_impossible_results() {
            // Client 0: Put(x, "1"), concurrent with Client 1: Put(x, "2").
            // Both claim to have returned None — impossible in any ordering,
            // because the second put must see the first's value.
            let history = vec![
                entry(
                    0,
                    Operation::Put {
                        key: key("x"),
                        value: val("1"),
                    },
                    0,
                    5,
                    None,
                ),
                entry(
                    1,
                    Operation::Put {
                        key: key("x"),
                        value: val("2"),
                    },
                    0,
                    5,
                    None,
                ),
            ];
            assert_eq!(check_linearizable(&history), CheckResult::Violation);
        }

        #[test]
        fn read_sees_value_before_write() {
            // Get returns "1" but the Put hasn't happened yet (strictly after).
            let history = vec![
                entry(0, Operation::Get { key: key("x") }, 0, 1, Some(val("1"))),
                entry(
                    0,
                    Operation::Put {
                        key: key("x"),
                        value: val("1"),
                    },
                    2,
                    3,
                    None,
                ),
            ];
            assert_eq!(check_linearizable(&history), CheckResult::Violation);
        }
    }
}
