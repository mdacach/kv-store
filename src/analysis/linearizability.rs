//! Brute-force linearizability checker.
//!
//! Determines whether a concurrent history of key-value operations is linearizable
//! by searching for a valid sequential ordering via recursive backtracking.
//!
//! The sequential reference is a standalone `BTreeMap<Key, Value>` that keeps
//! track of current values for each key.

use std::collections::BTreeMap;
use std::fmt;

use crate::analysis::history::HistoryEntry;
use crate::kv::{Key, Operation, OperationResult, Value};

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

fn try_linearize(
    entries: &[HistoryEntry],
    linearized: &mut Vec<bool>,
    order: &mut Vec<usize>,
    state: &mut BTreeMap<Key, Value>,
    best_order: &mut Vec<usize>,
) -> bool {
    if order.len() == entries.len() {
        return true;
    }

    for i in 0..entries.len() {
        if linearized[i] {
            continue;
        }

        let other_happens_before = entries.iter().enumerate().any(|(j, other)| {
            !linearized[j] && j != i && other.return_time < entries[i].invoke_time
        });
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

        if order.len() > best_order.len() {
            best_order.clone_from(order);
        }

        if try_linearize(entries, linearized, order, state, best_order) {
            return true;
        }

        linearized[i] = false;
        order.pop();
        revert(state, &entries[i].operation, &previous);
    }

    false
}

fn revert(state: &mut BTreeMap<Key, Value>, operation: &Operation, previous: &OperationResult) {
    match operation {
        Operation::Get { .. } => {}
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

        let blocked = entries.iter().enumerate().any(|(j, other)| {
            !in_prefix[j] && j != i && other.return_time < entries[i].invoke_time
        });
        if blocked {
            continue;
        }

        let reference_result = OperationResult(state.get(entries[i].operation.key()).cloned());
        if reference_result != entries[i].result {
            candidates.push(FailedCandidate {
                index: i,
                entry: entries[i].clone(),
                reference_result,
            });
        }
    }

    candidates
}
