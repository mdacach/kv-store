# PR Review Guide

This branch is intended to be reviewed commit by commit. The commits build the
Alloy Raft model from module structure through leader election refinements,
AppendEntries request/response handling, commit-index modeling, scenarios, and
safety assertions.

## Setup

Use the cleaned branch directly:

```bash
cd /Users/crs/Development/formal-methods/kv-store/worktrees/review-pr-7
git switch review-pr-7
git log --reverse --oneline origin/main..HEAD
```

If you need to compare against the pre-cleanup history, the backup branch is:

```bash
backup/review-pr-7-before-history-cleanup-20260505-120451
```

For each commit, inspect the patch in VS Code or the terminal:

```bash
git show --stat <commit>
git diff <commit>^ <commit> -- docs/alloy
code .
```

If you want to review one commit as an unstaged patch in a scratch worktree:

```bash
git worktree add ../review-pr-7-scratch origin/main
cd ../review-pr-7-scratch
git cherry-pick --no-commit <commit>
code .
```

When finished with that scratch patch, reset or remove only the scratch worktree:

```bash
git cherry-pick --abort 2>/dev/null || true
git reset --hard origin/main
cd ..
git worktree remove review-pr-7-scratch
```

## Commit Order

1. `c1b42d5` `Split Alloy model into submodules`
   Focus: module boundaries between `raft.als`, `safety.als`, `scenarios.als`,
   and `visualization.als`. Confirm the split keeps semantic facts in the model
   modules and leaves visualization as derived helper state.

2. `df4c3c5` `Add scaffold for log-replication`
   Focus: new log, entry, index, and AppendEntries vocabulary. Check whether the
   bounded one-entry-per-request abstraction is explicit and consistently used.

3. `99bf145` `Add "log freshness" rule when granting votes to candidates`
   Focus: RequestVote now includes candidate log metadata. Compare the Alloy
   `logUpToDate` relation against Raft's election restriction.

4. `4a9c104` `Track requested RequestVote peers`
   Focus: candidates track requested peers rather than response peers. Check
   that self-vote initialization, send bookkeeping, and vote-response assertions
   all describe the same concept.

5. `f60c927` `Move candidate state into Candidate signature`
   Focus: candidate-only fields move off `Node`. Confirm role transitions clear
   candidate bookkeeping when a node leaves `Candidate`.

6. `83b7b03` `Add leader replication bookkeeping`
   Focus: `nextIndex` and `matchIndex` are leader-only. Review role transition
   initialization and the safety checks that keep these indexes inside bounded
   logs.

7. `5ceeeb8` `Add client append and log helper cleanup`
   Focus: leader-local client append and readable log helpers. Confirm the entry
   freshness constraint prevents accidental aliasing with existing log entries.

8. `5102d4c` `Make safety and scenarios run with visualization helpers`
   Focus: visualization module wiring. Verify this commit does not add semantic
   constraints through visualization-only helpers.

9. `d39c922` `Add \`AppendEntries\` request transition`
   Focus: request construction from `nextIndex`, including `prevLogIndex`,
   `prevLogTerm`, carried entry, and `leaderCommit`.

10. `a2dc1b2` `Generalize higher-term RPC step-down`
    Focus: shared term-adoption behavior across RPC handlers. Check that
    same-term and higher-term cases stay distinct.

11. `bc1e493` `Add helpers for handling \`AppendEntries\``
    Focus: helper predicates for previous-log matching, conflict deletion,
    appending, and commit-index capping. Watch for helpers that accidentally
    constrain unrelated state.

12. `2419f73` `Handle AppendEntries requests`
    Focus: complete request handling for stale requests, mismatches, heartbeats,
    existing entries, conflicts, and new entries. This is one of the highest
    value semantic review points.

13. `fc49799` `Group protocol actions in the trace fact`
    Focus: trace readability. Confirm all intended actions remain reachable.

14. `8b4124c` `Introduce frame helper predicates`
    Focus: frame-condition reuse. Check that helpers are used only where all
    grouped state is actually unchanged.

15. `cb4626d` `Split AppendEntries handling into complete cases`
    Focus: case decomposition. Review whether cases are mutually clear and
    collectively cover the intended request outcomes.

16. `f574e3e` `Handle AppendEntries responses`
    Focus: leader response handling for success, failure, stale terms, and
    higher terms. Review `nextIndex` backoff and `matchIndex` advancement.

17. `eac9771` `Strengthen AppendEntries scenarios`
    Focus: executable examples for the request/response behavior introduced
    earlier. Check that scenario scopes are minimal but not overfit.

18. `a5560d9` `Add generic network action helpers`
    Focus: `send`, `discard`, and `reply` abstraction. Confirm the helpers
    improve readability without hiding message lifecycle details.

19. `5035866` `Route message handling through receive`
    Focus: network delivery structure. Confirm every handled message is consumed
    exactly once through the intended receive path.

20. `d891927` `Add Raft commit index modeling`
    Focus: `commitIndex`, leader commit advancement, follower commit updates,
    and safety coverage for committed entries.

21. `cb1f3c7` `Make edge-case AppendEntries scenarios executable`
    Focus: scenario reachability after commit-index constraints were added.
    Check whether any scope increases are justified.

22. `7d1c9b9` `Consolidate successful AppendEntries handling`
    Focus: reuse between success cases. Confirm the shared helper still makes
    the heartbeat, existing-entry, conflict-repair, and new-entry cases readable.

23. `8b11715` `Use ordering min for AppendEntries commit cap`
    Focus: simpler capping of follower commit index to the minimum of leader
    commit and latest matched index.

24. `0b799ec` `Show commit advancement in Alloy traces`
    Focus: visualization-only commit depiction. Confirm no model behavior
    depends on these helpers.

25. `ef57ec2` `Deduplicate visualization edge helpers`
    Focus: visualization helper reuse and edge labeling. This should be a
    low-risk cleanup commit.

26. `265f13a` `Assert committed log entries remain stable`
    Focus: safety assertion for committed entry stability. Compare it against
    the intended Raft safety property and the bounded model assumptions.

## Review Questions

- Does each semantic commit introduce one coherent modeling idea?
- Are all frame conditions explicit enough to show what does not change?
- Are helper predicates descriptive, or do they hide important state changes?
- Do request and response handling match the Raft TLA+ spec at the same level of
  abstraction, especially around log matching, conflict repair, and commit
  advancement?
- Are scenario scope increases required for reachability, or are they masking an
  accidental overconstraint?
- Does `visualization.als` remain derived presentation logic rather than part of
  the protocol semantics?

## Verification

Run the full safety check after reviewing semantic groups, and again after any
history rewrite:

```bash
alloy exec -q -t none -o - -c 'check*' docs/alloy/safety.als
```

Run the high-signal scenarios after reviewing AppendEntries handling:

```bash
alloy exec -q -t none -o - -c 'appendEntriesPrevMismatchRejectTrace' docs/alloy/scenarios.als
alloy exec -q -t none -o - -c 'appendEntriesConflictRepairTrace' docs/alloy/scenarios.als
alloy exec -q -t none -o - -c 'appendEntriesResponseBackoffTrace' docs/alloy/scenarios.als
```

If you change a reviewed commit, prefer creating a replacement or fixup commit
immediately after the commit under review, then re-run the relevant Alloy command
before moving on.
