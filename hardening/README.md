# Mo Hardening Tests

This directory contains stress and safety tests that exercise concurrency, callbacks,
method calls, and partially implemented runtime surfaces.

## Green executable suite

Run this in normal validation:

```sh
mo test hardening/test/concurrency_test.mo
```

The green suite currently covers:

- mutex-protected raw shared counters across multiple threads
- rwlock write/read ordering
- int and bool channel send/receive/close behavior
- function channels carrying named functions and closures
- `TaskQueue4` accepting more jobs than workers and shutting down
- `std/async.spawn/join` with moved captures
- thread move closures owning heap-backed struct fields
- callback values: named functions, closures, and function-valued struct fields
- method-call route-style registration mutating function-valued fields

## TDD regression specs

`hardening/tdd/*.mo` started as known-failing hardening specs and is now kept as
regression coverage for bugs that were exposed during hardening:

- concurrent `AtomicInt` clones under threaded increments
- borrowed `String` values copied across `Channel<String>` without transferring ownership

Run it in normal validation as well:

```sh
mo test hardening/tdd/atomic_clone_test.mo
mo test hardening/tdd/string_channel_test.mo
```

`tests/hardening.rs` contains CLI-level safety expectations for thread captures,
channel sends, and current method/field ambiguity behavior.
