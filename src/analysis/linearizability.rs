//! Brute-force linearizability checker.
//!
//! Determines whether a concurrent history of key-value requests is linearizable
//! by searching for a valid sequential ordering via recursive backtracking.
//!
//! The sequential reference is a standalone `BTreeMap<Key, Value>` that keeps
//! track of current values for each key.

use std::collections::BTreeMap;
use std::fmt;

use crate::analysis::history::HistoryEntry;
use crate::kv::{Key, Request, Response, Value};

/// A history entry paired with its index in the original history slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedEntry {
    pub index: usize,
    pub entry: HistoryEntry,
}

/// An eligible request that couldn't be linearized, with the response the
/// reference would have produced at that point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedCandidate {
    pub index: usize,
    pub entry: HistoryEntry,
    pub reference_response: Response,
}

/// Result of a linearizability check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    /// A valid linearization exists. Contains the requests in
    /// linearization order, each paired with its index in the input history.
    Ok { linearization: Vec<IndexedEntry> },
    /// No valid linearization exists.
    Violation {
        /// The longest sequence of requests that could be linearized.
        linearized_prefix: Vec<IndexedEntry>,
        /// Reference state after applying the linearized prefix.
        state_at_failure: BTreeMap<Key, Value>,
        /// Requests that were eligible to be linearized next but whose
        /// responses didn't match the reference.
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
                        e.request,
                        e.response,
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
                    "Linearized prefix ({} of {} requests):",
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
                            e.request,
                            e.response,
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
                        e.client_id, e.request, e.response, fc.reference_response,
                    )?;
                }
                Ok(())
            }
        }
    }
}

/// Apply a request to the reference system (correctness oracle).
pub(crate) fn apply_to_reference(
    state: &mut BTreeMap<Key, Value>,
    request: &Request,
) -> Response {
    match request {
        Request::Put { key, value } => Response(state.insert(key.clone(), value.clone())),
        Request::Get { key } => Response(state.get(key).cloned()),
        Request::Delete { key } => Response(state.remove(key)),
    }
}

/// Check whether a history is linearizable.
///
/// Tries all valid orderings of concurrent requests via backtracking.
/// A request is "eligible" to be linearized next only if no un-linearized
/// request happens-before it (i.e., it has a strictly earlier return_time
/// than this request's invoke_time).
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
    // requests. It is still worthwhile to provide more information than that,
    // though, so let's output the linearizable prefix that we could find, and
    // what went wrong afterwards.

    // The best partial order is tracked throughout the linearizability
    // checking. Replaying that order in a clean state will give us the
    // reference state at the linearizability-failing point. The replay is done
    // after the fact so we do not need to keep both the best order and the
    // best-order-state during the checking.
    let mut state_at_failure = BTreeMap::new();
    for &i in &best_order {
        apply_to_reference(&mut state_at_failure, &entries[i].request);
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
/// candidate request is reverted by restoring the previous value at the
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

        // The chosen entry must be causally eligible:
        // no *other* remaining entry may finish before it starts.
        //
        // If some un-linearized j has return_time < this invoke_time,
        // then j must come before i in every valid linearization.
        let invoke_i = entries[i].invoke_time;
        let mut blocked = false;
        for j in 0..entries.len() {
            if linearized[j] || i == j {
                continue;
            }
            if entries[j].return_time < invoke_i {
                blocked = true;
                break;
            }
        }
        if blocked {
            continue;
        }

        // Check whether the request's observed response matches what the
        // sequential reference would produce at this point.
        let reference_response = apply_to_reference(state, &entries[i].request);
        if reference_response != entries[i].response {
            // Mismatch: restore state and skip.
            revert_request(state, &entries[i].request, reference_response);
            continue;
        }

        // Commit entry i into the current linearization prefix.
        linearized[i] = true;
        order.push(i);
        if order.len() > best_order.len() {
            *best_order = order.clone();
        }

        if try_linearize(entries, linearized, order, state, best_order) {
            return true;
        }

        // Backtrack entry i.
        order.pop();
        linearized[i] = false;
        revert_request(state, &entries[i].request, reference_response);
    }

    false
}

/// Undo one previously-applied request, given the response that request
/// observed at apply time.
fn revert_request(
    state: &mut BTreeMap<Key, Value>,
    request: &Request,
    reference_response: Response,
) {
    match request {
        Request::Put { key, .. } => {
            if let Some(prev) = reference_response.0 {
                state.insert(key.clone(), prev);
            } else {
                state.remove(key);
            }
        }
        Request::Get { .. } => {
            // Reads do not mutate state.
        }
        Request::Delete { key } => {
            if let Some(prev) = reference_response.0 {
                state.insert(key.clone(), prev);
            } else {
                state.remove(key);
            }
        }
    }
}

/// Compute the set of entries that were causally eligible immediately after
/// replaying `best_order`, but whose observed responses don't match the
/// reference state at that point.
fn compute_failed_candidates(
    entries: &[HistoryEntry],
    best_order: &[usize],
    state_at_failure: &BTreeMap<Key, Value>,
) -> Vec<FailedCandidate> {
    let mut linearized = vec![false; entries.len()];
    for &i in best_order {
        linearized[i] = true;
    }

    let mut failed = Vec::new();
    for i in 0..entries.len() {
        if linearized[i] {
            continue;
        }

        // Same eligibility rule as in the search.
        let invoke_i = entries[i].invoke_time;
        let mut blocked = false;
        for j in 0..entries.len() {
            if linearized[j] || i == j {
                continue;
            }
            if entries[j].return_time < invoke_i {
                blocked = true;
                break;
            }
        }
        if blocked {
            continue;
        }

        let mut state_copy = state_at_failure.clone();
        let reference_response = apply_to_reference(&mut state_copy, &entries[i].request);
        if reference_response != entries[i].response {
            failed.push(FailedCandidate {
                index: i,
                entry: entries[i].clone(),
                reference_response,
            });
        }
    }

    failed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(
        client: u8,
        request: Request,
        invoke_time: u64,
        return_time: u64,
        response: Response,
    ) -> HistoryEntry {
        HistoryEntry {
            client_id: crate::protocol::ClientID(client),
            request,
            invoke_time,
            return_time,
            response,
        }
    }

    fn k(s: &str) -> Key {
        Key(s.into())
    }

    fn v(s: &str) -> Value {
        Value(s.into())
    }

    mod linearizable {
        use super::*;

        fn indices(result: &CheckResult) -> Vec<usize> {
            match result {
                CheckResult::Ok { linearization } => {
                    linearization.iter().map(|e| e.index).collect()
                }
                CheckResult::Violation { .. } => panic!("expected linearizable history"),
            }
        }

        #[test]
        fn empty_history() {
            let result = check_linearizable(&[]);
            let CheckResult::Ok { linearization } = result else {
                panic!("empty history should be linearizable");
            };
            assert!(linearization.is_empty());
        }

        #[test]
        fn sequential_put_then_get() {
            let history = vec![
                h(
                    0,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    0,
                    1,
                    Response(None),
                ),
                h(
                    0,
                    Request::Get { key: k("x") },
                    2,
                    3,
                    Response(Some(v("1"))),
                ),
            ];

            let result = check_linearizable(&history);
            let CheckResult::Ok { linearization } = result else {
                panic!("history should be linearizable");
            };
            assert_eq!(
                linearization.iter().map(|e| e.index).collect::<Vec<_>>(),
                vec![0, 1]
            );
        }

        #[test]
        fn concurrent_puts() {
            let history = vec![
                h(
                    0,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    0,
                    5,
                    Response(None),
                ),
                h(
                    1,
                    Request::Put {
                        key: k("x"),
                        value: v("2"),
                    },
                    1,
                    4,
                    Response(Some(v("1"))),
                ),
                h(
                    2,
                    Request::Get { key: k("x") },
                    6,
                    7,
                    Response(Some(v("2"))),
                ),
            ];

            let result = check_linearizable(&history);
            let CheckResult::Ok { linearization } = result else {
                panic!("history should be linearizable");
            };
            assert_eq!(
                linearization.iter().map(|e| e.index).collect::<Vec<_>>(),
                vec![0, 1, 2]
            );
        }

        #[test]
        fn concurrent_requests_on_different_keys() {
            let history = vec![
                h(
                    0,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    0,
                    3,
                    Response(None),
                ),
                h(
                    1,
                    Request::Get { key: k("y") },
                    1,
                    2,
                    Response(None),
                ),
            ];

            assert_eq!(indices(&check_linearizable(&history)), vec![0, 1]);
        }

        #[test]
        fn put_get_delete_get_sequential() {
            let history = vec![
                h(
                    0,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    0,
                    1,
                    Response(None),
                ),
                h(
                    0,
                    Request::Get { key: k("x") },
                    2,
                    3,
                    Response(Some(v("1"))),
                ),
                h(
                    0,
                    Request::Delete { key: k("x") },
                    4,
                    5,
                    Response(Some(v("1"))),
                ),
                h(
                    0,
                    Request::Get { key: k("x") },
                    6,
                    7,
                    Response(None),
                ),
            ];

            assert_eq!(indices(&check_linearizable(&history)), vec![0, 1, 2, 3]);
        }
    }

    mod violations {
        use super::*;

        #[test]
        fn stale_read() {
            let history = vec![
                h(
                    0,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    0,
                    1,
                    Response(None),
                ),
                h(
                    1,
                    Request::Get { key: k("x") },
                    2,
                    3,
                    Response(None),
                ),
            ];

            let result = check_linearizable(&history);
            assert!(result.is_violation());

            let CheckResult::Violation {
                linearized_prefix,
                failed_candidates,
                ..
            } = result
            else {
                panic!("expected violation");
            };

            assert_eq!(linearized_prefix.len(), 1);
            assert_eq!(failed_candidates.len(), 1);
            assert_eq!(
                failed_candidates[0].reference_response,
                Response(Some(v("1")))
            );
        }

        #[test]
        fn read_sees_value_before_write() {
            let history = vec![
                h(
                    0,
                    Request::Get { key: k("x") },
                    0,
                    1,
                    Response(Some(v("1"))),
                ),
                h(
                    1,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    2,
                    3,
                    Response(None),
                ),
            ];

            let result = check_linearizable(&history);
            assert!(result.is_violation());

            let CheckResult::Violation {
                linearized_prefix,
                state_at_failure,
                failed_candidates,
            } = result
            else {
                panic!("expected violation");
            };

            assert!(linearized_prefix.is_empty());
            assert!(state_at_failure.is_empty());
            assert_eq!(failed_candidates.len(), 1);
            assert_eq!(failed_candidates[0].entry, history[0]);
            assert_eq!(failed_candidates[0].reference_response, Response(None));
        }

        #[test]
        fn display_stale_read() {
            let history = vec![
                h(
                    0,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    0,
                    1,
                    Response(None),
                ),
                h(
                    1,
                    Request::Get { key: k("x") },
                    2,
                    3,
                    Response(None),
                ),
            ];

            let result = check_linearizable(&history);
            let rendered = result.to_string();

            assert!(rendered.contains("Linearizability violation detected."));
            assert!(rendered.contains("Reference state at failure: {x: \"1\"}"));
            assert!(rendered.contains("history says None, reference says Some(\"1\")"));
        }

        #[test]
        fn concurrent_requests_impossible_responses() {
            let history = vec![
                h(
                    0,
                    Request::Put {
                        key: k("x"),
                        value: v("1"),
                    },
                    0,
                    5,
                    Response(None),
                ),
                h(
                    1,
                    Request::Put {
                        key: k("x"),
                        value: v("2"),
                    },
                    1,
                    4,
                    Response(Some(v("9"))),
                ),
            ];

            let result = check_linearizable(&history);
            assert!(result.is_violation());
        }
    }
}
