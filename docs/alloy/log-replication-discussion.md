# Raft Log Replication Alloy Discussion

This note collects the questions we should settle before extending the current
leader-election Alloy model with log replication.

## Scope

1. Are we adding only enough log structure to support the RequestVote log
   freshness rule, or are we modeling AppendEntries, follower log repair,
   commitment, and leader completeness too?
Answer: We're adding all of those, yes. This phase should be a complete model of Raft's core parts: leader-election and log-replication. Cluster membership changes and log compaction are OUT OF SCOPE.
2. Which properties should this phase prove?
   Good candidates include log matching, election safety with log freshness,
   leader completeness, committed-entry preservation, and state-machine safety
   if applied commands are modeled.
Answer: Ideally we check all properties covered in Raft's original paper.

## Log Representation

1. Do we need real client commands, or is it enough to model abstract entries
   with terms and ordered indexes?
Answer: Abstract entries are fine.
2. Should entries be first-class atoms, or should each node's log be modeled as
   an index-to-entry relation?
Answer: index-to-entry sounds better, but we can investigate both.
3. Should logs be required to be contiguous prefixes from the first index, or
   should gaps be representable so assertions catch them?
Answer: In the Raft sense, there shouldn't be gaps. So I'm thinking our model shouldn't allow gaps — but by checking properties, not by fabricating `fact`s. 

## RequestVote Freshness

1. What exact last-log metadata should RequestVoteRequest carry?
   The usual Raft fields are last log index and last log term.
Answer: usual Raft.
2. Should the freshness rule be encoded directly in vote-granting logic, or
   factored into a helper predicate that can be asserted independently?
Answer: helper predicate.
3. What assertion should demonstrate the new rule?
   A useful first one: a candidate with a stale log cannot collect a majority
   of granted votes and become leader.
Answer: investigate later.

## AppendEntries Fidelity

1. Should AppendEntries messages include the real Raft fields:
   prevLogIndex, prevLogTerm, entries, and leaderCommit?
Answer: Yes. We want fidelity to Raft.
2. Should AppendEntriesResponse include success or failure plus enough metadata
   for the leader to update replication bookkeeping?
Answer: Yes.
3. Should we model one entry per AppendEntries message first, or allow batches?
   One entry per message is easier to check and visualize.
Answer: One entry per message to start, if easier.

## Leader Bookkeeping

1. Do we model nextIndex and matchIndex for each leader/follower pair, or
   abstract them away and allow leaders to send any AppendEntries consistent
   with their own logs?
Answer: I believe Raft has them for each pair, so we should do the same.
2. If we model nextIndex and matchIndex, are they persistent node-local state in
   the Alloy model, or derived from successful responses?
Answer: Probably persistent? What does the paper use?
3. How much fidelity is needed to prove the target properties?
   Modeling bookkeeping is more realistic but increases the search space.
Answer: Increases to the search space are fine. So let's discuss on a case-by-case basis.

## Commit Semantics

1. Are we ready to introduce commitIndex?
Answer: In this part, yes. Not necessarily first.
2. Should the model include Raft's rule that leaders only directly commit log
   entries from their current term, with older entries committed indirectly?
Answer: Yes, otherwise the protocol is broken.
3. Should followers advance commitIndex only through leaderCommit in
   AppendEntries?
Answer: Yes.
4. Do we need applied state-machine commands, or is committed log safety enough
   for this phase?
Answer: committed log is enough.

## Network Semantics

1. Should the current global InFlight message set remain the only network
   abstraction?
Answer: Depends, you can change it if good reason.
2. Should this phase add message loss, duplication, or reordering explicitly?
   Reordering already comes naturally from nondeterministic delivery order;
   loss and duplication would need explicit transitions or relaxed message
   handling.
Answer: Not at first, but we can discuss in this part, yes.
3. Should stale AppendEntries and stale RequestVote messages be retained as
   useful counterexample sources?
Answer: What do you mean by "retained"? If that means keeping track of them forever, no, not worth it.

## Facts vs Assertions

1. Which log properties are protocol rules that belong inside transitions?
Answer: You choose.
2. Which log properties are safety claims that should remain assertions?
Answer: You choose.
3. Are there any tempting facts that would make traces cleaner but hide invalid
   transition behavior?
Answer: Don't know.

## Bounded Checking Strategy

1. What small scopes should be the default during development?
   Log replication will increase search cost, so useful starting scopes may be
   3 Node, 3-4 Term, a small number of log indexes, and tightly bounded
   messages.
Answer: Waiting a while for it to complete is fine, we don't need to minimize scopes.
2. Which scenarios should exist alongside assertions?
   Examples: successful replication, conflicting follower repair, stale
   candidate vote denial, and committed entry surviving a leader change.
Answer: Investigate later.
3. Should scenarios be split so each one exercises a single protocol path?
Answer: Maybe.

## Visualization

1. Should we add event tags for AppendEntries send, AppendEntries handling,
   response handling, commit advancement, and client append?
Answer: Follow the scheme in the leader-election part of the model, so yes.
2. Which derived relations would make traces readable?
   Useful candidates include per-node log edges, replicated entry agreement,
   leader-to-follower AppendEntries edges, and commitIndex markers.
Answer: Your answer.

## Suggested First Milestone

Add log entries, last-log metadata, and the RequestVote freshness rule first.
Then add an assertion showing that a stale-log candidate cannot win an election.

After that passes in small scopes, add AppendEntries and commit behavior
incrementally.
