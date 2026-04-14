# Rust Documentation Guide

This guide distills current Rust documentation guidance from official Rust sources, the Rust API Guidelines, and community practice. It is written as a project-facing "how to write good docs" reference for Rust crates.

## Short version

Write documentation at two levels:

- `rustdoc` for reference: crate docs, module docs, item docs, examples, error and panic behavior, safety contracts, and searchable links.
- Long-form prose for onboarding and understanding: tutorials, task guides, architecture notes, and design rationale.

Do not try to make one document do every job.

## What "good Rust documentation" means

Good Rust documentation lets a reader answer four questions quickly:

1. What is this crate or item for?
2. How do I use it right now?
3. What can go wrong?
4. Why is it designed this way?

Official Rust guidance strongly emphasizes clear crate-level docs, documentation for public items, and runnable examples. Community practice adds an important nuance: `rustdoc` is excellent reference documentation, but it is awkward as the only home for tutorials or narrative guides.

## Mechanical Rustdoc conventions

### Use the right comment form

- Use `///` for item docs.
- Use `//!` only for crate-level or module-level docs.
- Prefer line doc comments over block doc comments.
- Prefer `//` over `/* ... */` for ordinary code comments.

### Use one-line summaries first

The first line matters because rustdoc reuses it in search results and module overviews.

- Start with a one-sentence summary.
- Keep it concise.
- Describe what the item does, not its full implementation.

Recommended pattern:

```rust
/// Returns the next committed operation in log order.
```

Rust's official docs do not prescribe a strict grammar rule here, but standard-library and rustdoc examples consistently use short present-tense summaries such as `Returns...`, `Creates...`, and `Parses...`. Follow that pattern consistently.

### Prefer present tense and direct descriptions

For public API docs, use:

- Present tense: `Returns`, `Creates`, `Checks`, `Parses`.
- Direct statements over notes-to-self.
- Consistent terminology for your domain model.

Avoid:

- Future tense: `This function will return...`
- Commentary about the implementation when the reader needs the contract.
- Restating types already visible in the signature.

Bad:

```rust
/// This function takes a `Request` and returns a `Response`.
```

Better:

```rust
/// Applies a client request to the state machine and returns the resulting response.
```

## What to write about

### Crate-level docs

Your crate docs should answer:

- What problem does this crate solve?
- Who is it for?
- What are the main concepts?
- What is the fastest correct way to get started?
- What features, constraints, or platform assumptions matter?

For this project, crate-level docs should probably explain:

- The role of the crate: simulation and analysis of a key-value store / protocol.
- The main building blocks: `runtime`, `simulator`, `analysis`, `visualization`.
- A minimal end-to-end example.
- What "linearizability" means in the context of the crate.
- Whether the crate is for experimentation, teaching, verification, production embedding, or some mix.

### Module-level docs

Module docs should explain boundaries and responsibilities.

Good module docs answer:

- Why does this module exist?
- What belongs here?
- What does not belong here?
- How does it relate to neighboring modules?

For example:

- `runtime`: execution model, node behavior, message handling.
- `simulator`: event scheduling, history capture, determinism or seeding rules.
- `analysis`: correctness checks, assumptions, interpretation of results.
- `visualization`: what artifacts are generated and for whom.

### Type and trait docs

Document the semantics, not only the shape.

Good type docs include:

- The invariant the type maintains.
- Ownership or lifetime expectations when they matter semantically.
- Whether values are opaque handles, data records, or protocol/state carriers.
- Any important valid or invalid states.

### Function and method docs

A useful default shape is:

1. One-line summary.
2. Details that are not obvious from the signature.
3. One or more examples.
4. Structured sections when needed.

Structured sections to use:

- `# Examples`
- `# Errors`
- `# Panics`
- `# Safety`

Also document, when relevant:

- Preconditions and postconditions.
- Performance characteristics.
- Ordering guarantees.
- Determinism, randomness, or seeding behavior.
- Concurrency guarantees.

## Examples are part of the API

Rust's official guidance is unusually strong here: examples should be common, realistic, and runnable.

Prefer examples that show:

- Why someone would use the API, not just how to call it.
- Complete copy-pasteable usage.
- Assertions about output or state.

Use doctest attributes precisely:

- Default fenced Rust blocks when the code should compile and run.
- `no_run` when it should compile but not execute in docs.
- `should_panic` when demonstrating panic behavior.
- `compile_fail` for invalid uses or safety boundaries.

Useful techniques:

- Hide setup lines with `#`.
- Use `assert!` and `assert_eq!` so examples verify behavior.
- Avoid `ignore` unless there is no better option.

This project already has an `examples/` directory. That is a good base for both narrative docs and API examples. If desired, Rustdoc also has an unstable scraped-examples feature that can pull example usage from `examples/` into generated docs.

## Comments inside code

Public docs and internal comments serve different purposes.

Use ordinary code comments for:

- Why this approach was chosen.
- Invariants that are not obvious from types.
- Subtle algorithmic constraints.
- Protocol assumptions.
- Concurrency or memory-order reasoning.

Do not use comments to narrate obvious code line by line.

Good internal comments answer:

- Why is this safe?
- Why is this ordering necessary?
- Why is this branch possible?
- What would break if this changed?

### Unsafe code comments

If `unsafe` appears, document it twice when needed:

- Public `unsafe fn` docs get a `# Safety` section describing caller obligations.
- Individual `unsafe` blocks get a `SAFETY:` comment explaining why the block is sound here.

That split is important:

- `# Safety` explains the public contract.
- `SAFETY:` explains the local proof.

## Links, searchability, and discoverability

Use Rustdoc's strengths.

- Use backticks for code terms.
- Use intra-doc links like [`Node`] and [`Simulator::step`] whenever they help navigation.
- Use `#[doc(alias = "...")]` for common alternate names only when users are likely to search for them.
- Hide irrelevant implementation details with `#[doc(hidden)]` when they would confuse users.

Do not turn docs into a wall of links. Link the important nouns and concepts.

## What not to do

- Do not duplicate the function signature in prose.
- Do not mix tutorial, reference, and design rationale into a single page.
- Do not treat README, crate docs, and API docs as identical text.
- Do not write examples that only compile in theory but are never tested.
- Do not use comments to compensate for unclear names or poor factoring when refactoring is practical.

## Recommended documentation stack for Rust projects

### For small crates

Usually enough:

- Strong `README.md`
- Strong crate-level `//!` docs
- Public item docs with examples
- `examples/` directory

### For medium or complex crates

Usually worth adding:

- Task-oriented guides
- Architecture and explanation docs
- Release notes / changelog
- CI checks for docs quality

This crate is already beyond "tiny utility crate" size. It has multiple subsystems and a domain model. That means relying on rustdoc alone is likely too narrow.

## Divio's four documentation types in Rust projects

Divio separates documentation into tutorials, how-to guides, reference, and explanation. That maps well onto Rust.

### 1. Tutorials

Goal: teach a beginner by leading them through an end-to-end success path.

For this project, tutorial candidates:

- Build and run the first simulation.
- Generate and inspect a message trace.
- Run a linearizability check on a simple scenario.

Tutorial rules:

- Beginner-first.
- Fully reproducible.
- Minimal explanation.
- Visible success at each step.

### 2. How-to guides

Goal: help an already-oriented user solve a specific task.

For this project:

- How to add a new scenario.
- How to seed the simulator for reproducible failures.
- How to render a trace visualization.
- How to add a new invariant or checker.
- How to write a regression test for a discovered bug.

How-to rules:

- Start from the practical question.
- Give steps.
- Skip long theory.

### 3. Reference

Goal: describe the machinery accurately.

For Rust projects, this is where `rustdoc` shines.

For this project, reference includes:

- API docs for modules, types, traits, and functions.
- Public constants and type aliases.
- CLI or configuration reference, if any.
- Output formats, event structures, and file formats.

### 4. Explanation

Goal: provide background, tradeoffs, and rationale.

For this project:

- Why the crate is split into runtime / simulator / analysis / visualization.
- What assumptions the linearizability checker makes.
- How the event/history model relates to the protocol model.
- Why certain simplifications are acceptable in this codebase.
- Tradeoffs between realism, determinism, and teachability.

Explanation is also the right place for:

- design notes
- architecture overviews
- model limitations
- known non-goals

## Should we introduce the other documentation types?

Yes, but not mechanically and not all at once.

For a Rust project like this one, I would recommend:

1. Keep rustdoc as the reference layer.
2. Add a small long-form docs layer for tutorials, how-to guides, and explanation.
3. Start with the missing user questions, not a full documentation bureaucracy.

So: yes, we should introduce the other documentation types, because this project has multiple concepts and workflows that do not fit cleanly into API reference alone.

But: no, we should not force a large documentation system immediately if the project is still changing rapidly. Start with a few high-value pages.

## A pragmatic rollout plan for this repository

### Phase 1: make rustdoc strong

- Add crate-level docs in `src/lib.rs`.
- Add `//!` docs to major modules.
- Add item docs and examples to the public API.
- Enable `#![warn(missing_docs)]` once the public surface is ready.
- Consider Clippy lints for `missing_errors_doc`, `missing_panics_doc`, and `missing_safety_doc`.

### Phase 2: add guide-shaped docs

Use `mdBook` or a simple `docs/` tree.

Suggested structure:

```text
docs/
  tutorials/
    first-simulation.md
    first-linearizability-check.md
  how-to/
    add-a-scenario.md
    debug-a-failing-seed.md
    render-a-trace.md
  explanation/
    architecture.md
    event-model.md
    linearizability-model.md
```

If this grows, promote it to an `mdBook`.

### Phase 3: connect the layers

- Link from `README.md` to crate docs and guide docs.
- Link from crate-level docs to tutorials/how-to/explanation.
- Link from explanation docs back to the relevant API reference.

## Practical house style

Recommended team rules:

- Every public item gets docs unless trivially self-evident.
- Every non-trivial public item gets an example.
- Every fallible public API documents `# Errors`.
- Every panicking public API documents `# Panics`.
- Every public `unsafe` API documents `# Safety`.
- Every `unsafe` block gets a `SAFETY:` comment.
- Crate and module docs explain concepts and boundaries.
- Internal comments explain why, invariants, and tradeoffs.
- README explains when and why to use the crate.
- Crate docs explain how to start using it.
- Long-form docs explain tasks and rationale.

## Community takeaways worth adopting

Two community opinions line up well with the official material:

- `README.md` and crate docs should overlap, but they should not be identical. README is best for "what/why"; crate docs are better for "how".
- Rustdoc is strongest as reference documentation. When a project needs tutorials or narrative guides, a separate book or docs section is usually the better fit.

Those are not formal Rust rules, but they match what many mature Rust projects do in practice.

## Sources

Official Rust sources:

- Rust API Guidelines, Documentation: <https://rust-lang.github.io/api-guidelines/documentation.html>
- The rustdoc book, How to write documentation: <https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html>
- The rustdoc book, Documentation tests: <https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html>
- The rustdoc book, Rustdoc lints: <https://doc.rust-lang.org/rustdoc/lints.html>
- The rustdoc book, Scraped examples: <https://doc.rust-lang.org/rustdoc/scraped-examples.html>
- Rust Style Guide, comments and doc comments: <https://doc.rust-lang.org/style-guide/>
- Rust Reference, comments: <https://doc.rust-lang.org/reference/comments.html>
- Standard library developers guide, Writing documentation: <https://std-dev-guide.rust-lang.org/development/how-to-write-documentation.html>
- Standard library developers guide, Safety comments policy: <https://std-dev-guide.rust-lang.org/policy/safety-comments.html>
- Standard library developers guide, Doc alias policy: <https://std-dev-guide.rust-lang.org/policy/doc-alias.html>
- Clippy lints: <https://rust-lang.github.io/rust-clippy/master/>
- mdBook introduction: <https://rust-lang.github.io/mdBook/>

Community sources:

- Rust forum, Best practice for documenting crates (README vs doc comments): <https://users.rust-lang.org/t/best-practice-for-documenting-crates-readme-md-vs-documentation-comments/124254>
- Rust forum, Any consensus on including long-form documentation in a crate?: <https://users.rust-lang.org/t/any-consensus-on-including-long-form-documentation-in-a-crate/18113>
- Rust forum, Functions on docs.rs: <https://users.rust-lang.org/t/functions-on-docs-rs/136971>
- Rust forum, How to create Tutorial/Guide/API documentation in Rust?: <https://users.rust-lang.org/t/how-to-create-tutorial-guide-api-documentation-in-rust/77139>
- Rust blog, The Rust Libz Blitz: <https://blog.rust-lang.org/2017/05/05/libz-blitz/>

Inspiration:

- Divio Documentation System: <https://docs.divio.com/documentation-system/>
