# Raft Log Replication Implementation Plan

This plan decomposes the log-replication work into atomic implementation steps.
Each step should be implemented, verified with Alloy in the smallest useful
scope, and committed before moving to the next step.

The target is a faithful core Raft model: leader election plus log replication.
Cluster membership changes, snapshots, and log compaction remain out of scope.

## Step 1: Add Log Vocabulary And Helpers

Goal: introduce log state without changing protocol behavior.

Model changes:

- Add finite ordered `Index` atoms for bounded log positions.
- Add `Entry` atoms with at least an entry term and an abstract payload/value.
- Add per-node log state as an index-to-entry relation.
- Add helpers for occupied indexes, last log index, last log term, and entry
  lookup.
- Initialize all logs as empty.
- Update `stutter` and all existing transitions with unchanged log state.

Properties and scenarios:

- Assert logs have no gaps as a safety property produced by transitions.
- Assert each node has at most one entry per index.
- Keep all existing leader-election assertions passing.

Uncertainty:

- Prefer index-to-entry logs first. If Alloy syntax or performance becomes
  awkward, revisit first-class sequence-like entry atoms.

## Step 2: Add RequestVote Log Freshness

Goal: make election voting depend on Raft's last-log freshness rule.

Model changes:

- Add `lastLogIndex` and `lastLogTerm` payload fields to
  `RequestVoteRequest`.
- Populate those fields when a candidate sends a vote request.
- Add a `logUpToDate` helper matching Raft: higher last-log term wins; if terms
  are equal, greater or equal last-log index wins.
- Require `logUpToDate` before granting a vote.
- Deny vote requests with stale logs.

Properties and scenarios:

- Assert any granted vote implies the candidate log metadata is at least as
  up-to-date as the receiver's log.
- Keep election safety checks passing.

Uncertainty:

- Avoid the overbroad property "stale candidate cannot win"; stale relative to
  one server is not enough to prevent election.

## Step 3: Add Candidate Response Bookkeeping

Goal: align vote response tracking more closely with Raft and Ongaro's TLA+
spec.

Model changes:

- Add `votesResponded` per node.
- Reset `votesResponded` on timeout.
- Prevent sending duplicate RequestVote requests to peers that have already
  responded in the current term.
- Add responders to `votesResponded` when responses are handled.
- Keep `votesGranted` as the granted-vote subset.

Properties and scenarios:

- Assert `votesGranted` is always a subset of `votesResponded` plus the
  candidate's self-vote policy, depending on the final encoding.
- Keep `LeadersRequireMajority` and `AtMostOneLeaderPerTerm` passing.

Uncertainty:

- The current Alloy model self-votes during timeout. Ongaro's TLA+ model sends
  a self RequestVote instead. Keep the current self-vote behavior unless it
  starts obscuring properties.

## Step 4: Add Leader Replication Bookkeeping

Goal: add leader-local replication progress state.

Model changes:

- Add `nextIndex` as leader-to-follower index state.
- Add `matchIndex` as leader-to-follower acknowledged index state.
- Initialize `nextIndex` to the first index and `matchIndex` to no index.
- Reset leader bookkeeping on timeout or when a node becomes leader as needed.
- In `becomeLeader`, set the new leader's `nextIndex` for each peer to one
  past the leader's last log index and `matchIndex` to empty/no index.

Properties and scenarios:

- Assert leaders do not have `matchIndex` beyond their own last log index.
- Keep existing election assertions passing.

Uncertainty:

- Alloy has no integer sequence indexes by default; "one past last index" must
  be represented with ordered `Index` atoms and explicit edge cases.

## Step 5: Add Client Append

Goal: allow a leader to create log entries.

Model changes:

- Add a `clientAppend` transition for leaders.
- Append one fresh abstract entry at the first free index after the leader's
  current last log index.
- Stamp the entry with the leader's current term.
- Leave messages, election state, and replication bookkeeping unchanged.

Properties and scenarios:

- Add a scenario where a leader eventually has a non-empty log.
- Assert leader append-only: a leader cannot modify or remove existing entries
  in its own log while it remains leader in the same term.

Uncertainty:

- Entry payload values are abstract. If state-machine safety needs visible
  values later, the existing payload field should be enough.

## Step 6: Add AppendEntries Message Shape And Send Transition

Goal: model leaders sending log replication requests without handling them yet.

Model changes:

- Add `AppendEntriesRequest` with `prevLogIndex`, `prevLogTerm`, optional single
  entry payload, entry index, and `leaderCommit`.
- Add `AppendEntriesResponse` with `success` and `matchIndex` payload fields.
- Add `sendAppendEntriesRequest` for a leader and peer.
- Populate previous-log metadata from `nextIndex`.
- Send at most one entry, following Ongaro's one-entry TLA+ decomposition.
- Add AppendEntries events and basic visualization helpers.

Properties and scenarios:

- Add a scenario where a leader appends an entry and sends AppendEntries.
- Assert sent AppendEntries previous-log metadata agrees with the leader log.

Uncertainty:

- Empty heartbeat messages may need a representation for "no entry" and "no
  match index"; use `lone` fields or sentinel atoms consistently.

## Step 7: Generalize Term Handling

Goal: share newer-term and stale-response behavior across all RPC types.

Model changes:

- Factor newer-term step-down into a generic helper used by RequestVote and
  AppendEntries handling.
- Make newer-term RPCs update `currentTerm`, move the receiver to follower, and
  clear any term-local vote as required by Raft.
- Add stale-response dropping for response messages with older terms.
- Keep current RequestVote behavior equivalent after refactoring.

Properties and scenarios:

- Generalize the step-down assertion to any newer-term RPC.
- Assert stale responses do not change protocol state except the network.

Uncertainty:

- The existing RequestVote handler updates term and handles the request in one
  atomic transition. Ongaro's TLA+ model separates `UpdateTerm`; either encoding
  is acceptable if behavior remains faithful.

## Step 8: Handle AppendEntries Requests

Goal: implement follower-side log replication and repair.

Model changes:

- Reject AppendEntries with stale terms.
- Step candidates down to followers on same-term AppendEntries.
- Check `prevLogIndex` and `prevLogTerm` against the receiver log.
- On mismatch, reply failure without changing the log.
- On matching previous entry, accept empty heartbeats.
- On matching previous entry plus a new entry:
  - If the receiver already has the same entry at that index, leave the log as
    is and reply success.
  - If the receiver has a conflicting entry at that index, remove the conflict
    and suffix according to Raft.
  - If the receiver log ends immediately before that index, append the entry.
- Advance follower commit index from `leaderCommit` when commit state exists.

Properties and scenarios:

- Add scenarios for successful append, heartbeat acceptance, conflict rejection,
  and conflict repair.
- Assert accepted AppendEntries preserves the log matching precondition.

Uncertainty:

- Ongaro's TLA+ removes one conflicting entry per transition; Raft describes
  deleting the conflicting entry and all that follow. One-step suffix deletion
  is simpler in Alloy; one-entry deletion is closer to the TLA+ atomic
  decomposition.

## Step 9: Handle AppendEntries Responses

Goal: implement leader-side replication progress updates.

Model changes:

- On successful response, set `matchIndex` for the follower to the response
  match index and set `nextIndex` to the following index.
- On failed response, decrement `nextIndex` toward the first index.
- Consume handled responses from `InFlight`.
- Ignore or drop stale responses through the generic stale-response transition.

Properties and scenarios:

- Add a scenario where a failed response causes retry with a lower `nextIndex`.
- Add a scenario where successful replication advances `matchIndex`.
- Assert `matchIndex` never exceeds the leader's last log index.

Uncertainty:

- Decrementing `nextIndex` requires representing the first index and the
  predecessor relation carefully in bounded ordered indexes.

## Step 10: Add Commit Semantics

Goal: model committed log positions.

Model changes:

- Add per-node `commitIndex`, represented as no committed index or one committed
  index per node.
- Initialize all commit indexes as empty/no index.
- Add `advanceCommitIndex` for leaders.
- Require a quorum `matchIndex` agreement for a candidate commit index.
- Require the entry being directly committed by a leader to be from the
  leader's current term.
- Let followers advance commit index from `leaderCommit` in accepted
  AppendEntries, capped by the follower's last log index.

Properties and scenarios:

- Add a scenario where an entry is replicated to a majority and committed.
- Assert commit index never exceeds the node's last log index.
- Assert commit index is monotonic unless a deliberate TLA+-style exception is
  chosen for old duplicated AppendEntries.

Uncertainty:

- Ongaro's TLA+ allows follower `commitIndex` to decrease in one duplicated
  message case and notes it does not affect safety. Prefer monotonic commit
  indexes unless this blocks faithful behavior.

## Step 11: Add Core Safety Assertions

Goal: check the main Raft safety properties in bounded Alloy scopes.

Model changes:

- Add `LeaderAppendOnly`.
- Add `LogMatching`.
- Add `LeaderCompleteness`.
- Add committed-entry agreement as the committed-log equivalent of
  state-machine safety.
- Keep the existing election assertions.

Properties and scenarios:

- Run all checks in small default scopes.
- Add larger optional check commands if small scopes pass quickly.

Uncertainty:

- Leader completeness may require a history variable like Ongaro's `elections`
  or `allLogs`. Add history only if the direct assertion is too weak or too
  expensive.

## Step 12: Add Network Loss And Duplication

Goal: expand network nondeterminism after the base protocol is stable.

Model changes:

- Add `dropMessage` transition that removes any in-flight message.
- Add `duplicateMessage` transition if the set-based network can represent
  duplicate payloads with distinct message atoms.
- If set-based duplication is awkward, document the limitation or switch to a
  message multiplicity encoding in a separate commit.

Properties and scenarios:

- Keep all safety assertions passing with drop enabled.
- Keep all safety assertions passing with duplication enabled if implemented.
- Add a scenario showing retry after message drop if feasible.

Uncertainty:

- The current `InFlight` set cannot contain the same message atom twice. True
  duplication may require payload-equivalent but atom-distinct messages or a bag
  encoding, which may be too large for this phase.
