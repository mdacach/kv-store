# Review Requests

This file tracks the latest in-code `REVIEW` requests and how they were resolved.

## Completed

- [x] Add crate-level documentation in `src/lib.rs`, including a project overview and the role of each public module.
- [x] Improve documentation across the project, especially around simulator responsibilities, request tracking, and public API semantics.
- [x] Document why `Key` and `Value` are string wrappers in `src/kv.rs`.
- [x] Extract the simulator event log into a dedicated `EventLog` type with `EventEntry` records outside `src/simulator/mod.rs`.
- [x] Rename simulator request tracking from `History` to `RequestHistory` and document that it exists for linearizability checking.
- [x] Clarify the difference between the event log and request history in simulator documentation.
- [x] Keep the stop-and-wait workload model documented on client registration and response handling.
- [x] Improve simulator method docs for scheduling, node selection, ticking, routing, quiescence, and log formatting.
- [x] Rename `send` to `send_message` for clarity and simplify next-request dispatch to return `Option<Message>`.
- [x] Add tracing warnings for unexpected simulator conditions instead of silently returning in those cases.
- [x] Rename `LogEntry` to `EventEntry` and document why `Send` records both send and delivery times.
