# Mo Delivery Loop

This is the operating contract for driving the Mo roadmap to completion.

Use [roadmap.md](roadmap.md) as the single source of truth. Do not create a
second active roadmap, and do not treat historical roadmap stubs as active
planning documents.

## Loop Contract

Each iteration must:

1. Re-read the active roadmap.
2. Select the highest-priority unblocked roadmap item.
3. Define the smallest meaningful deliverable for that item.
4. Audit the current implementation and tests.
5. Add or update tests first where possible.
6. Implement the slice.
7. Update examples or demos when public behavior changes.
8. Update roadmap/reference docs when behavior or status changes.
9. Run focused verification.
10. Run full verification.
11. Continue to the next unblocked item.

Do not stop at planning when implementation is feasible. Do not move to a new
roadmap item while the current slice has failing tests.

## Slice Definition

A valid slice is small enough to verify and large enough to reduce roadmap risk.

It must have:

- a clear compiler, runtime, stdlib, or user-facing behavior,
- focused tests or updated existing tests,
- no unrelated refactors,
- docs updated if public behavior changed,
- examples/demo updated if the feature is user-visible,
- full suite green before continuing.

## Priority Rules

Default roadmap order:

1. Core semantics.
2. Allocation and collections.
3. Async runtime.
4. Networking and HTTP.
5. Packages.
6. Hardening and portability.

Within a phase, choose the item that unlocks the most later work.

If an item is blocked:

1. State the blocker concretely.
2. Identify the dependency.
3. Add a regression/TODO test only if it preserves useful knowledge.
4. Move to the prerequisite, not unrelated work.

A blocker is not that the work is large. A blocker means the slice cannot be
implemented correctly until another capability exists.

## Verification Gates

Every code-changing iteration must pass:

```sh
cargo fmt --check
cargo test
```

Also run focused tests before the full suite:

```sh
cargo test <focused-filter>
```

Runtime/std/server/network changes need at least one executable compile/run
test through `tests/cli.rs` or an equivalent `mo build`/run smoke.

Server-visible changes should also build the demo:

```sh
./target/debug/mo build examples/demo/pokemon_server.mo -o /tmp/mo_pokemon_server
```

Load-sensitive server/runtime changes should run the Pokémon load script and
record results when the measurement is relevant.

## Documentation Gates

After each completed slice:

- update [roadmap.md](roadmap.md) if status or sequencing changed,
- update [reference.md](reference.md) if public language or stdlib behavior changed,
- update [ownership-roadmap.md](ownership-roadmap.md) only when the ownership model changes,
- avoid duplicating active plans in other files.

Docs must answer:

- what is true now,
- what remains,
- what is next,
- what acceptance means.

## Demo Policy

Update `examples/demo/pokemon_server.mo` when a feature becomes part of the
public story:

- ownership-visible APIs,
- package/import changes,
- route/server changes,
- async/networking changes,
- collection-backed server behavior,
- measurable runtime/memory behavior.

Do not add demo noise for purely internal compiler changes.

## No Shortcuts

The loop must not:

- claim future behavior that is only metadata,
- replace missing semantics with documentation,
- skip tests for executable behavior,
- mark a milestone complete when only a stub exists,
- hide failures by narrowing the test surface,
- leave examples or docs describing behavior that is not implemented,
- perform broad rewrites unrelated to the active slice.

Prototype slices are acceptable only when they are named honestly as prototype
slices and have executable coverage for the behavior they claim.

## Whole-Roadmap Completion

The loop is complete only when the near-term queue in [roadmap.md](roadmap.md)
is exhausted or every remaining item is explicitly marked future/non-blocking.

The expected end state includes:

- coherent public `Str`/`String` ownership,
- generalized drop behavior for compound owned values,
- stronger enum/pattern/`Result`/`Option` typing,
- real `alloc` boundary,
- more generic collections,
- real future polling and `async.block_on`,
- async TCP backed by executor/event readiness,
- typed HTTP request/response,
- collection-backed routes/middleware,
- package manifests or a stable package-root story,
- full suite green,
- demo current,
- roadmap updated.
