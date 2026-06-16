# Subagent Control Workflow

This workflow is for parallel implementation of bounded slices from the active
[Mo roadmap](roadmap.md) under the [delivery loop](delivery-loop.md).

The parent agent owns integration, sequencing, final review, and final test
gates. Subagents own disjoint work slices with explicit file ownership.

## Global Rules

- Every implementation slice starts with tests or updates existing tests before implementation is considered complete.
- Every subagent must state changed files in its final message.
- Subagents must not revert other work.
- Subagents must assume the worktree may be dirty and integrate with existing edits.
- Subagents should keep changes within assigned ownership.
- The parent agent reviews each returned patch before integrating or continuing.
- The parent agent runs the final full suite when code changes land: `cargo fmt --check && cargo test`.

## Test Gates

- Parser/type/IR changes: targeted unit tests plus the full suite.
- Backend/codegen changes: at least one native executable CLI test plus the full suite.
- Runtime/std changes: native executable CLI test using the public API plus the full suite.
- Server/demo changes: focused compile/run smoke plus the full suite.
- Docs-only changes: inspect links and keep terminology aligned with [roadmap.md](roadmap.md).

## Workstream Ownership

### Core Semantics

Owns:

- `src/ast.rs`
- `src/parser.rs`
- `src/hir.rs`
- `src/resolve.rs`
- `src/typeck.rs`
- `src/ownership.rs`
- `src/borrow.rs`
- `src/dropck.rs`
- `src/ir.rs`
- parser/type/ownership/drop/IR tests

Typical slices:

- public `String`/`Str` semantics,
- recursive/path-sensitive drop glue,
- enum and pattern typing,
- `Result`/`Option` typing,
- method dispatch.

### Backend And Runtime

Owns:

- `src/backend/cranelift.rs`
- `src/runtime.rs`
- `core/`
- runtime-focused CLI tests

Typical slices:

- value representation changes,
- call/return ABI changes,
- allocator/runtime counters,
- platform symbol support.

### Stdlib And Allocation

Owns:

- `std/string.mo`
- `std/buffer.mo`
- `std/vec.mo`
- `std/map.mo`
- `std/box.mo`
- `std/shared.mo`
- future `alloc/`
- std library tests and compile examples

Typical slices:

- `alloc` package boundary,
- generic collection behavior,
- collection drops,
- slice/indexing behavior.

### Async, Threads, And Networking

Owns:

- `std/thread.mo`
- `std/task.mo`
- `std/async.mo`
- `std/event.mo`
- `std/async_tcp.mo`
- `std/net.mo`
- `src/threadck.rs`
- async/thread/network tests and examples

Typical slices:

- real `Future<T>` representation,
- executor and wakeups,
- typed nonblocking TCP,
- kqueue backend,
- structured network errors.

### HTTP, Packages, And Demo

Owns:

- `std/http.mo`
- `lib/express.mo`
- `lib/pokemon.mo`
- `lib/pokemon_server.mo`
- `lib/json.mo`
- `src/package.rs`
- `examples/demo/`
- server/package tests and load tooling

Typical slices:

- typed HTTP request/response,
- dynamic route tables,
- middleware chains,
- package manifests,
- benchmark/demo updates.

## Parent Agent Integration Loop

1. Select the next highest-priority unblocked item from [roadmap.md](roadmap.md).
2. Split the work into one local critical path plus optional disjoint subagent slices.
3. Spawn subagents only when file ownership is clear and the work can proceed independently.
4. Continue local work while subagents run.
5. Review returned changes for scope, tests, and consistency.
6. Run targeted tests for each integrated slice.
7. Run `cargo fmt --check && cargo test`.
8. Update [roadmap.md](roadmap.md) when roadmap status changes.

There is no separate active queue in this file. The near-term queue in
[roadmap.md](roadmap.md) is the source of truth.
