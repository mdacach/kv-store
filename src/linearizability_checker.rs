//! Brute-force linearizability checker.
//!
//! Determines whether a concurrent history of key-value operations is linearizable
//! by searching for a valid sequential ordering via recursive backtracking.
//!
//! The sequential reference is a standalone `BTreeMap<Key, Value>` that keeps
//! track of current values for each key.

use std::collections::BTreeMap;
use std::fmt;

use crate::history::HistoryEntry;
use crate::node::{Key, Operation, OperationResult, Value};

/// A history entry paired with its index in the original history slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedEntry {
    pub index: usize,
    pub entry: HistoryEntry,
}

/// An eligible operation that couldn't be linearized, with the result the
/// reference would have produced at that point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedCandidate {
    pub index: usize,
    pub entry: HistoryEntry,
    pub reference_result: OperationResult,
}

/// Result of a linearizability check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    /// A valid linearization exists. Contains the operations in
    /// linearization order, each paired with its index in the input history.
    Ok { linearization: Vec<IndexedEntry> },
    /// No valid linearization exists.
    Violation {
        /// The longest sequence of operations that could be linearized.
        linearized_prefix: Vec<IndexedEntry>,
        /// Reference state after applying the linearized prefix.
        state_at_failure: BTreeMap<Key, Value>,
        /// Operations that were eligible to be linearized next but whose
        /// results didn't match the reference.
        failed_candidates: Vec<FailedCandidate>,
    },
}

impl CheckResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, CheckResult::Ok { .. })
    }

    pub fn is_violation(&self) -> bool {
        matches!(self, CheckResult::Violation { .. })
    }
}

impl fmt::Display for CheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckResult::Ok { linearization } => {
                writeln!(f, "Linearizable.")?;
                if linearization.is_empty() {
                    return Ok(());
                }
                writeln!(f)?;
                writeln!(f, "Linearization order:")?;
                for (pos, ie) in linearization.iter().enumerate() {
                    let e = &ie.entry;
                    writeln!(
                        f,
                        "  {}. {} {} -> {}    [t={}..{}]",
                        pos + 1,
                        e.client_id,
                        e.operation,
                        e.result,
                        e.invoke_time,
                        e.return_time,
                    )?;
                }
                Ok(())
            }
            CheckResult::Violation {
                linearized_prefix,
                state_at_failure,
                failed_candidates,
            } => {
                writeln!(f, "Linearizability violation detected.")?;
                writeln!(f)?;

                let total = linearized_prefix.len() + failed_candidates.len();
                writeln!(
                    f,
                    "Linearized prefix ({} of {} operations):",
                    linearized_prefix.len(),
                    total,
                )?;
                if linearized_prefix.is_empty() {
                    writeln!(f, "  (none)")?;
                } else {
                    for (pos, ie) in linearized_prefix.iter().enumerate() {
                        let e = &ie.entry;
                        writeln!(
                            f,
                            "  {}. {} {} -> {}    [t={}..{}]",
                            pos + 1,
                            e.client_id,
                            e.operation,
                            e.result,
                            e.invoke_time,
                            e.return_time,
                        )?;
                    }
                }
                writeln!(f)?;

                write!(f, "Reference state at failure: ")?;
                if state_at_failure.is_empty() {
                    writeln!(f, "{{}}")?;
                } else {
                    write!(f, "{{")?;
                    for (i, (k, v)) in state_at_failure.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{k}: \"{v}\"")?;
                    }
                    writeln!(f, "}}")?;
                }
                writeln!(f)?;

                writeln!(f, "Could not linearize:")?;
                for fc in failed_candidates {
                    let e = &fc.entry;
                    writeln!(
                        f,
                        "  - {} {}: history says {}, reference says {}",
                        e.client_id, e.operation, e.result, fc.reference_result,
                    )?;
                }
                Ok(())
            }
        }
    }
}

/// Apply an operation to the reference system (correctness oracle).
pub(crate) fn apply_to_reference(
    state: &mut BTreeMap<Key, Value>,
    operation: &Operation,
) -> OperationResult {
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
/// operation happens-before it (i.e., it has a strictly earlier return_time
/// than this operation's invoke_time).
pub fn check_linearizable(entries: &[HistoryEntry]) -> CheckResult {
    let mut linearized = vec![false; entries.len()];
    let mut order = Vec::with_capacity(entries.len());
    let mut state = BTreeMap::new();
    let mut best_order: Vec<usize> = Vec::new();

    if try_linearize(
        entries,
        &mut linearized,
        &mut order,
        &mut state,
        &mut best_order,
    ) {
        let linearization = order
            .into_iter()
            .map(|i| IndexedEntry {
                index: i,
                entry: entries[i].clone(),
            })
            .collect();
        return CheckResult::Ok { linearization };
    }

    // At this point, we know there's no suitable linearizable ordering of the
    // operations. It is still worthwhile to provide more information than that,
    // though, so let's output the linearizable prefix that we could find, and
    // what went wrong afterwards.

    // The best partial order is tracked throughout the linearizability
    // checking. Replaying that order in a clean state will give us the
    // reference state at the linearizability-failing point. The replay is done
    // after the fact so we do not need to keep both the best order and the
    // best-order-state during the checking.
    let mut state_at_failure = BTreeMap::new();
    for &i in &best_order {
        apply_to_reference(&mut state_at_failure, &entries[i].operation);
    }

    let failed_candidates = compute_failed_candidates(entries, &best_order, &state_at_failure);

    let linearized_prefix = best_order
        .into_iter()
        .map(|i| IndexedEntry {
            index: i,
            entry: entries[i].clone(),
        })
        .collect();

    CheckResult::Violation {
        linearized_prefix,
        state_at_failure,
        failed_candidates,
    }
}

/// Returns true if a valid linearization was found.
///
/// `state` is mutated in place across recursive calls; on backtrack, the
/// candidate operation is reverted by restoring the previous value at the
/// targeted key. This avoids cloning the state at every candidate, which
/// could be considerably expensive.
fn try_linearize(
    entries: &[HistoryEntry],
    linearized: &mut Vec<bool>,
    order: &mut Vec<usize>,
    state: &mut BTreeMap<Key, Value>,
    best_order: &mut Vec<usize>,
) -> bool {
    // If all entries have been linearized, we're done.
    if order.len() == entries.len() {
        return true;
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

        let reference_result = state.get(entries[i].operation.key());
        if reference_result != entries[i].result.0.as_ref() {
            continue;
        }

        let previous = apply_to_reference(state, &entries[i].operation);

        linearized[i] = true;
        order.push(i);

        // Track the longest prefix we've successfully linearized to enrich
        // diagnostics in case of failure.
        if order.len() > best_order.len() {
            best_order.clone_from(order);
        }

        if try_linearize(entries, linearized, order, state, best_order) {
            return true;
        }

        // Backtracking: undo the bookkeeping and revert the state mutation.
        linearized[i] = false;
        order.pop();
        revert(state, &entries[i].operation, &previous);
    }

    false
}

/// Undo a mutation made by [`apply_to_reference`].
fn revert(state: &mut BTreeMap<Key, Value>, operation: &Operation, previous: &OperationResult) {
    match operation {
        // Get is read-only — nothing to undo.
        Operation::Get { .. } => {}
        // Put and Delete both either inserted or removed at the key.
        // Restore the previous value, or remove if there was none.
        Operation::Put { key, .. } | Operation::Delete { key } => match &previous.0 {
            Some(v) => {
                state.insert(key.clone(), v.clone());
            }
            None => {
                state.remove(key);
            }
        },
    }
}

/// Identify eligible operations at the failure point and what the reference
/// would have returned for each.
fn compute_failed_candidates(
    entries: &[HistoryEntry],
    prefix: &[usize],
    state: &BTreeMap<Key, Value>,
) -> Vec<FailedCandidate> {
    let mut in_prefix = vec![false; entries.len()];
    for &i in prefix {
        in_prefix[i] = true;
    }

    let mut candidates = Vec::new();
    for i in 0..entries.len() {
        if in_prefix[i] {
            continue;
        }
        let other_happens_before = entries.iter().enumerate().any(|(j, other)| {
            !in_prefix[j] && j != i && other.return_time < entries[i].invoke_time
        });
        if other_happens_before {
            continue;
        }
        // This operation was eligible but it couldn't be linearized. Compute
        // what the reference would have returned.
        let reference_result = OperationResult(state.get(entries[i].operation.key()).cloned());
        candidates.push(FailedCandidate {
            index: i,
            entry: entries[i].clone(),
            reference_result,
        });
    }
    candidates
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

        /// Extract just the indices from a linearization for easier assertion.
        fn indices(result: &CheckResult) -> Vec<usize> {
            match result {
                CheckResult::Ok { linearization } => {
                    linearization.iter().map(|ie| ie.index).collect()
                }
                _ => panic!("expected Ok"),
            }
        }

        #[test]
        fn empty_history() {
            let result = check_linearizable(&[]);
            assert_eq!(indices(&result), Vec::<usize>::new());
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
            let result = check_linearizable(&history);
            assert_eq!(indices(&result), vec![0, 1]);
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
            let result = check_linearizable(&history);
            assert_eq!(indices(&result), vec![0, 1, 2]);
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
            assert!(check_linearizable(&history).is_ok());
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
            assert_eq!(indices(&check_linearizable(&history)), vec![0, 1, 2, 3]);
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
            let result = check_linearizable(&history);
            // The put linearizes fine; the get fails because the reference
            // says Some("1") but the history recorded None.
            let CheckResult::Violation {
                linearized_prefix,
                state_at_failure,
                failed_candidates,
            } = &result
            else {
                panic!("expected Violation");
            };
            assert_eq!(linearized_prefix.len(), 1);
            assert_eq!(linearized_prefix[0].index, 0);
            assert_eq!(state_at_failure, &BTreeMap::from([(key("x"), val("1"))]));
            assert_eq!(failed_candidates.len(), 1);
            assert_eq!(failed_candidates[0].index, 1);
            assert_eq!(
                failed_candidates[0].reference_result,
                OperationResult(Some(val("1")))
            );
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
            let result = check_linearizable(&history);
            // Whichever put goes first succeeds (returns None), but the
            // second put would return Some(_), which doesn't match None.
            let CheckResult::Violation {
                linearized_prefix,
                failed_candidates,
                ..
            } = &result
            else {
                panic!("expected Violation");
            };
            assert_eq!(linearized_prefix.len(), 1);
            assert_eq!(failed_candidates.len(), 1);
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
            let result = check_linearizable(&history);
            // The get is first (happens-before the put), but the reference
            // returns None for a get on an empty store — doesn't match Some("1").
            let CheckResult::Violation {
                linearized_prefix,
                state_at_failure,
                failed_candidates,
            } = &result
            else {
                panic!("expected Violation");
            };
            assert!(linearized_prefix.is_empty());
            assert!(state_at_failure.is_empty());
            // Only the get is eligible (put happens after).
            assert_eq!(failed_candidates.len(), 1);
            assert_eq!(failed_candidates[0].index, 0);
            assert_eq!(failed_candidates[0].reference_result, OperationResult(None));
        }

        #[test]
        fn display_stale_read() {
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
            let result = check_linearizable(&history);
            let output = result.to_string();
            assert!(output.contains("Linearizability violation detected."));
            assert!(output.contains("1 of 2 operations"));
            assert!(output.contains("Put(x, \"1\") -> None"));
            assert!(output.contains("x: \"1\""));
            assert!(output.contains("history says None, reference says Some(\"1\")"));
        }
    }
}
