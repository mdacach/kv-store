# PR Review Guide

This PR is easiest to review commit by commit rather than as one aggregate diff.

Recommended order:

1. `fc9a061` `Refactor simulator around multi-node routing`
2. `a5035c3` `Refresh example scenarios for routed multi-node behavior`
3. `f0bb327` `Replace single-node tests with multi-node coverage`

Suggested review flow:

1. Start with `fc9a061`.
   Focus on the behavioral change: the simulator now owns multiple nodes, routes each client operation to a chosen node, records per-operation routing, and removes the old `Server` wrapper. Verify the message flow changes in `src/simulator/mod.rs`, the protocol simplification in `src/protocol.rs`, and the new simulator support modules in `src/simulator/history.rs` and `src/simulator/log.rs`.
2. Review `a5035c3` next.
   Treat this as a consumer update. Check that the example scenarios and trace actor labels now reflect node-level routing and intentionally demonstrate stale reads and divergence.
3. Finish with `f0bb327`.
   Confirm the old single-node assumptions were intentionally removed and replaced with phase-2 expectations: completion, determinism for fixed seeds, seeded stale-read behavior, and a stable linearizability violation regression.

Questions worth asking while reviewing:

- Does every client request still produce exactly one matching response in the recorded history?
- Is the routing decision recorded and exposed only where needed for tests and visualization?
- Are the new seeded tests asserting stable behaviors, or are they overfitting to incidental log details?
- Is removing `Server` actually simplifying the model, or hiding responsibilities that should remain explicit?
- Do the visualization/example changes reflect simulator semantics faithfully, especially now that actors include multiple nodes?

Useful commands:

```bash
git log --reverse --oneline main..HEAD
git show fc9a061
git show a5035c3
git show f0bb327
cargo test
```

If you prefer GitHub review, use the commit view first, then fall back to the full Files Changed tab only after the commit-by-commit pass.
