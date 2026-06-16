# Mo Roadmap

This is the single active roadmap for Mo.

The old milestone queue has been completed as executable prototype slices. The
next phase is no longer about proving that the compiler can run a TCP-backed
demo. It is about turning the prototype into a coherent language implementation
that can support larger programs without special cases.

## Current Standing

Mo is an executable macOS/aarch64 systems-language prototype with:

- Parser, AST, HIR, IR, name resolution, target filtering, and native Cranelift builds.
- Ownership, borrow, drop, and thread-safety checks for the covered language surface.
- Public `Str`/`String` bridge, automatic owned-string cleanup, `Box`, `Vec`, `Map`, `Shared`, buffers, channels with std int/string tests, tasks, threads, sync primitives, typed unsafe pointers, and runtime memory counters.
- `std/fs`, `std/process`, `std/path`, `std/int`, `std/bytes`, `std/string`, `std/net`, `std/event`, `std/async`, `std/async_tcp`, and `std/http` executable slices, with passing std test files wired into the CLI gate.
- Relative imports, `std/`, `core/`, `alloc/`, `lib/`, and `mo.toml` dependency package roots, public/private import checks, namespace aliases, and module-qualified object symbols.
- Userland `lib/json`, `lib/pokemon`, `lib/express`, and `lib/pokemon_server` packages.
- Native Pokémon REST and threadpool server smokes with real TCP, Vec-backed route registration, route count tracking, and ordered before-request middleware callbacks.

The suite is green at the time this roadmap was written:

```text
cargo fmt --check
cargo test
```

The CLI compile/run suite currently has 189 passing tests.

## Product Goal

Mo should become a small, readable, native systems language with:

- deterministic cleanup,
- no garbage collector,
- safe default ownership and borrowing,
- explicit shared ownership,
- typed unsafe boundaries,
- practical concurrency,
- async networking,
- a standard library implemented mostly in Mo,
- enough package structure to build real services.

The benchmark application remains the Pokémon REST API because it exercises
imports, JSON, filesystem state, strings, TCP, HTTP, callbacks, routes, tasks,
threads, async facades, and memory cleanup in one small program.

## Guiding Rules

- Keep compiler-known behavior limited to low-level bricks.
- Prefer Mo standard/userland code over compiler magic whenever the current type system can support it.
- Add generality only when a focused executable test needs it.
- Keep every milestone backed by compile/run tests, semantic tests, or both.
- Keep the demo program current when a user-visible language/library feature lands.

The execution methodology is defined in [delivery-loop.md](delivery-loop.md).
It is part of the roadmap contract: no milestone is complete without tests,
docs, examples when relevant, and full verification.

## Phase 1: Make The Core Semantics Honest

Goal: reduce special cases in ownership, types, and drops so the language model is coherent.

### 1. Complete Public String Ownership

Current state: string literals type as borrowed `Str` in frontend type
checking; `String` means an owned heap string at public API boundaries;
owned `String` values can be read through `Str` parameters without moving; and
`String.from`, `String.new`, `String.clone`, `String.concat`, stdlib fallbacks,
JSON/path/fs helpers, integer parsing, channel string send-by-reference, TCP/async TCP text writes, HTTP response
rendering helpers, route handlers, `std/io.Writer`, reference examples, and the demo/library code
now make borrowed-vs-owned intent explicit with read-only text accepted as `&Str`.
Std C/read-style buffer externs in fs/http/net/io/process now borrow caller-owned
string buffers as `&String` instead of taking ownership by value. IR
owned-return inference now uses public by-value `String` parameter facts, so
wrappers that return an owned parameter preserve automatic cleanup at the call
site. By-value `String` parameters, direct `String` return annotations, and
direct `String` local annotations now lower as owned IR facts, while `Str`,
`&Str`, and `&String` remain borrowed string values. Struct fields, enum
payloads, and container storage still use the existing string layout bridge
with recursive drop glue until storage ownership can be represented without
backend special cases. The compiler still uses internal `OwnedString` markers
as a backend representation bridge for drops and low-level string-producing
intrinsics, but ordinary function lowering, closure lowering, and function type
boundaries now use public `String` return facts consistently, including
string-returning indirect callback calls. Owned-return propagation now only
applies to functions declared as returning public by-value `String`; `Str`
wrapper returns and legacy known string-producing names declared as `Str` remain
borrowed IR values. Function signature lowering no longer runs a recursive
known-name owned-string return analyzer; declared return types provide the
ownership fact directly, with the existing `buffer.finish` storage bridge kept
as the only special return adjustment. Function values remain copy-like handles, while
function-typed callback parameters now carry ownership modes, so by-value
`String` callback arguments move and `&Str` callback arguments borrow in the
ownership checker.

Next work:

- Replace more internal `OwnedString` expression/local inference with normal
  type/ownership facts now that by-value parameters and borrowed references are
  separated in IR.
- Continue auditing new std/userland APIs so read-only parameters use `&Str` and owning APIs use `String`.

Acceptance:

- Owned string moves and borrowed `Str` calls behave consistently across direct calls, wrappers, structs, enum payloads, extern returns, and route handlers.
- Memory-counter tests still prove owned strings are freed.

### 2. Generalize Drop Glue

Current state: owned strings, buffers, boxes, vectors, maps, shared handles, task queues, typed TCP listener/stream owners, and custom `Drop` paths have executable coverage. Early returns from branch-local scopes, ordinary `if` branch fallthrough, nested block expressions, value-producing block expressions assigned to locals or returned from functions, `return if` branch-value expressions, value-producing `if` expressions assigned to locals, and block-bodied match arms now drop inner locals in reverse lexical order before outer locals; block final-expression moves, conservative `if` branch moves, conservative `match` arm moves, conservative `while` body moves, conservative `for` body moves, conservative `unsafe` block moves, and loop-body moves before direct or nested `break` now propagate to the parent ownership scope when later code can observe the move; loop `break`/`continue` exits drop active loop-body owned locals before jumping; `?` error paths now emit IR drops for live owned locals before returning; scalar `return match` expressions over owned enum locals now compute into a temporary before dropping storage-only matched enum values; scalar returns and function-valued returns that read owned aggregate locals now compute into a temporary before dropping those aggregates; enum and struct returns constructed from read-only uses of owned locals now materialize into a return temporary before those locals are dropped; string returns that reference owned locals now use a return temporary before cleanup; call arguments inside larger expressions now use signature-aware move/drop planning, so moved enum locals in `mo test` assertions are not dropped again at test exit; block and `if` expression final-value ownership is inferred for drop planning; consumed block final expressions now propagate moves of outer owned locals into drop planning, so the original local is not scheduled for a second drop; owned local reassignment evaluates the new value before cleaning up the previous value; recursive backend drop glue now frees owned `String` storage nested inside concrete struct fields, enum payloads, annotated and inferred generic struct specializations such as `Holder<String>`, and generic enum specializations such as `Maybe<String>`; typed `TcpListener` and `TcpStream` locals now close automatically at scope exit unless explicitly closed first; and moving fields out of owned aggregates now reports an explicit diagnostic instead of being treated as a read. Drop planning is still not fully path-sensitive for every compound expression shape.

Next work:

- Complete path-sensitive return cleanup for any remaining compound expression forms after branch, nested block, loop-exit, and match-arm coverage.
- Replace remaining manual destroy/close requirements where ownership can own cleanup; raw fd APIs, shared channel handles, task-queue lifecycle sequencing, and aliased app/sync handles remain intentionally manual or blocked.
- Make partial-initialization behavior explicit in diagnostics when syntax support expands.

Acceptance:

- Drops run exactly once for nested owned fields, early returns, reassignment, moves, and thread/task captures.
- Tests cover structs, enums, vectors, maps, boxes, shared handles, and user `Drop` impls.

### 3. Finish Type Checking Fundamentals

Current state: type checking rejects many obvious errors and supports useful executable programs. Enum constructors now check positional payload arity and types for concrete payloads and for generic payloads when an expected enum type is available from annotations, returns, assignments, fields, or call parameters. Generic enum constructors also infer direct payload type arguments without an expected type, so values such as `Some(41)` carry `Option<Int>` into match payload bindings. Match arms bind positional payload variables with the matched enum's payload types, including substitution from annotated or inferred generic enum values such as `Option<Int>`, and match patterns now reject missing or extra payload bindings for the selected variant. Statement-form enum matches now lower to Cranelift switch/tag dispatch with executable payload-binding coverage. Enum-returning direct and indirect calls, direct enum returns, hinted enum constructor payloads, and pointer-sized indirect callback returns are covered in IR. `std/option` and `std/result` now provide importable public `Option<T>` and `Result<T, E>` enums backed by executable std tests. `std/option` includes `is_some`, `is_none`, `unwrap_or`, `map`, `and_then`, and `or_else`; `std/result` includes `is_ok`, `is_err`, `unwrap_or`, `map`, `and_then`, `map_err`, and `or_else`, with executable `Int` and owned-`String` coverage. The `?` operator now unwraps `Result<T, E>` and `Option<T>`, rejects non-propagation operands, and checks enclosing return compatibility. `return`, `break`, and `continue` now produce `Never` internally, so diverging `if` and `match` branches do not poison joined expression types. Generic function calls substitute explicit type arguments and infer omitted type arguments from expected return context and argument types. Method calls now resolve through the receiver type across type-body methods, interface methods, and receiver-compatible module functions, with diagnostics for known receivers.

Next work:

- Continue expanding `Result`/`Option` combinators beyond the current map/chain/recovery helpers where focused executable coverage needs them.

Acceptance:

- User-defined generic containers and functions type-check without std-specific lowering.
- Pattern matches reject payload type mismatches, constructor arity mistakes, and impossible payload bindings.
- Method resolution is deterministic and diagnostic quality is good.

## Phase 2: Make Allocation And Collections Real

Goal: move from concrete executable slices to a reusable allocation-backed stdlib.

### 4. Establish `alloc`

Current state: allocation exists through `core/unsafe` bricks. The first `alloc/` package root is available, with `alloc/string.mo` owning string allocation/copy/concat/int/byte/free helpers and `std/string.mo` delegating its public API through that boundary. `alloc/buffer.mo` owns the raw string-backed buffer allocation, byte load/store, and free helpers used by `std/buffer.mo`. `alloc/box.mo` owns Box cell allocation, cell load/store, string pointer storage, and cell free helpers used by `std/box.mo`. `alloc/vec.mo` owns Vec slot allocation, slot load/store, string pointer storage, legacy handler and typed request-handler function-pointer storage, element string free, and slot free helpers used by `std/vec.mo`. `alloc/map.mo` owns the current string/string map storage policy over Vec-backed keys and values for `std/map.mo`. Public `std/map.destroy_string_string` is covered as an explicit consuming cleanup. The reference now defines the ownership/free contracts for existing `alloc/string`, `alloc/buffer`, `alloc/box`, `alloc/vec`, and `alloc/map` APIs.

Next work:

- Continue moving raw allocation details out of public std APIs.
- Keep the alloc ownership/free contracts current as new storage helpers are added.

Acceptance:

- Public collection APIs live at stable package paths.
- Unsafe allocation remains isolated behind `core`/`alloc` internals.

### 5. Generalize `Vec`, `Map`, `Buffer`, And Slices

Current state: `Vec<Int>`, `Vec<String>`, legacy `Vec<fn(Int, &Str) -> Int>` callbacks, typed middleware `Vec<fn(Int, &http.Request, &Str) -> Int>` callbacks, `Map<String, String>`, fixed/growing buffer paths, first-class `std/buffer.StringBuilder` and `std/buffer.ByteBuffer` owner types over current buffer storage, an executable borrowed `std/slice.ByteSlice` value, and `Slice<T>` type parsing exist in slices. `Vec<Int>`, `Vec<String>`, and callback vectors have standalone or route-table executable coverage for generic push/get/length/capacity/destroy behavior, with `vec.new<T>`, `vec.data<T>`, `vec.push<T>`, `vec.get<T>`, `vec.length<T>`, `vec.capacity<T>`, and `vec.destroy<T>` specializing through the existing concrete helpers for `Int`, `String`, legacy handlers, and typed middleware; the demo capacity probe also exercises generic `Vec<Int>` push/read calls. `Map<String, String>` has executable generic put/get/length/destroy coverage through public `std/map` tests, with `map.get<K,V>`, `map.length<K,V>`, and `map.destroy<K,V>` specializing through the current string/string storage helpers; explicit generic map and vec destroy calls are recognized by drop planning so they are not followed by automatic collection drops. `Buffer` has public std tests for string/byte/int appends plus growth past initial capacity, `StringBuilder` has std, compile/run memory, auto-drop, IR, and drop-check coverage for append, append_byte, append_int, finish, capacity, remaining, and explicit destroy behavior, and `ByteBuffer` has std, compile/run memory, auto-drop, IR, and drop-check coverage for bounded push/get/set, length/capacity/remaining, growth, finish, and explicit destroy behavior. `std/slice.ByteSlice` now stores borrowed `Str` backing through a distinct lowered borrowed-string field type, with std, compile/run, IR, parser, and type-check coverage for `from_str`, `subslice`, `len`, `is_empty`, bounded `get`, and `expr[index]` syntax lowering through bounded `slice.get`. `lib/express.App` now stores route methods, route paths, and response handler pointers in Vec-backed tables; before-request middleware callbacks use the generic Vec facade. `std/http.HeaderMap` now provides a collection-backed string/string header store over the current Vec-backed map storage, with executable tests for standalone put/get/destroy and for request-owned parsed benchmark headers.

Next work:

- Make `Vec<T>` and `Map<K, V>` generic enough for arbitrary parsed request headers, request bodies, and richer non-handler route metadata.
- Generalize indexing beyond `ByteSlice` with first-class `Slice<T>` lowering and mutable indexing.
- Factor the shared buffer backing into a lower-level public `alloc.Buffer` contract once allocator ownership is generalized.

Acceptance:

- Route tables and HTTP headers use normal collections instead of fixed slots.
- Collection drop tests cover owned elements.

## Phase 3: Make Async Real

Goal: replace immediate async execution with real future polling and event-loop wakeups.

### 6. Future Representation And Polling

Current state: async functions record future metadata, `.await` preserves values in immediate execution, and `std.async.block_on(Int)` is an identity boundary.

Next work:

- Define `Future<T>` layout and poll contract.
- Lower async functions into resumable state machines.
- Store locals across suspension safely.
- Drop partially completed futures correctly.

Acceptance:

- `.await` suspends and resumes rather than executing immediately.
- Borrow-across-await checks match executable behavior.
- Future cancellation drops initialized owned locals.

### 7. Executor, Wakers, Timers

Current state: `std.async.spawn/join` wraps pthread-backed `fn() -> ()` tasks; `std/event` wraps `select`; `std/async_tcp` waits then performs synchronous operations.

Next work:

- Add executor task queues for futures.
- Add wakeup registration from event readiness.
- Add timers/sleep.
- Move `async_tcp` from readiness-gated synchronous helpers to future-returning operations.

Acceptance:

- `async.block_on` runs a future to completion.
- Async TCP accept/read/write can interleave multiple connections without one thread per request.

## Phase 4: Make Networking And HTTP General

Goal: grow beyond benchmark routes while keeping the userland/server layering.

### 8. TCP And Event Backend

Current state: typed `TcpListener`/`TcpStream`, readiness waits, and loopback smokes work over `select`.

Next work:

- Add nonblocking typed listener/stream constructors.
- Replace `select` internals with a kqueue backend on macOS while preserving `std/event` API.
- Add structured network errors instead of negative integer sentinels.

Acceptance:

- TCP APIs return typed results.
- Event tests cover readable, writable, timeout, and closed-fd behavior.

### 9. HTTP Request/Response

Current state: `std/http.read_request` returns a typed `Request` value with benchmark method ID, owned method name, owned path, route ID derived from the parsed path, parsed `Content-Length`, owned body string data, and request-owned Vec-backed string/string storage for arbitrary syntactically valid header lines. `std/http.request_header_count`, `std/http.request_header`, and `std/http.request_destroy` expose that parsed header store with explicit cleanup; lookup preserves exact header names and duplicate names use the latest stored value, and `request_destroy` also releases the owned body. `std/http.read_body` remains as a lower-level compatibility helper for callers that have not consumed the request with `read_request`, with targeted serial body-reader, request-line, arbitrary parsed-header, and typed request-body examples. `std/http.HeaderMap` also provides a standalone collection-backed string/string header store with owned string put/get/destroy semantics. Express now calls `read_request` in serial and threadpool serving paths, routes from the typed request method/path instead of the benchmark `route_id`, passes `&http.Request` into middleware and route handlers, accepts typed `http.Response` return values from route handlers, and writes those responses centrally. `request_route` remains a lightweight route-only compatibility parser for lower-level callers that still need it. `std/http.Response` carries typed status, owned body, owned content-type, and owned Vec-backed response headers; response rendering computes content length from the typed body and writes custom headers before the connection close header; and status helper functions now cover 200, 201, 400, 404, and 500 with targeted typed-response coverage. Existing direct write helpers remain compatibility shims.

Next work:

- Continue generalizing body readers and response helpers beyond the benchmark request/response shapes.
- Add non-string body representations and richer response header policy, including duplicate/override rules for built-in headers.

Acceptance:

- Server handlers return typed `Response`. Done for current Express route handlers.
- Existing Pokémon server behavior remains green.

### 10. Dynamic Router And Middleware

Current state: `lib/express.App` tracks route count through Vec-backed method/path/handler tables and supports multiple before-request middleware callbacks in deterministic registration order. Middleware uses `fn(Int, &http.Request, &Str) -> Int`; route handlers use `fn(Int, &http.Request, &Str) -> http.Response`, so they can inspect parsed method/path/body/header data and return typed responses while Express owns socket writes. Serial and threadpool serving paths now match registered routes against `Request.method` and `Request.path`, so routes such as `/custom` work even when the benchmark `route_id` is `0`. The legacy `route_id` remains available on `Request` for compatibility.

Next work:

- Add parameterized path segments and richer route metadata once string/slice support is broad enough.

Acceptance:

- Routes are stored in `Vec`/`Map`-backed tables.
- Middleware order is deterministic and tested.

## Phase 5: Make Packages Scalable

Goal: allow multi-package userland programs without ad hoc roots.

### 11. Package Manifests

Current state: `std/`, `core/`, `alloc/`, and repository `lib/` roots are built in. The loader also discovers the nearest ancestor `mo.toml` and resolves package roots from `[dependencies]` entries such as `math = "packages/math"` and current-target `[target.macos.dependencies]` entries, so imports like `math/answer` can load local packages by manifest name. Package manifests can also declare target-gated native linker inputs with sections such as `[native.macos.aarch64]`, including `static_libraries`, `objects`, `library_paths`, `libraries`, and `link_args`; those inputs are collected transitively and passed to the final `cc` link step.

Next work:

- Expand manifest metadata beyond package name/root and dependency roots.
- Generalize target-specific manifest rules beyond the current macOS target.

Acceptance:

- A sample app imports a local package by manifest name.
- Duplicate transitive imports remain de-duplicated.

### 12. Module And Visibility Polish

Current state: public/private import checks, explicit namespace aliases, and
module-level struct field visibility work. Selected brace-import diagnostics
report missing names, available public exports, and private item guidance.
Namespace alias misses report the unavailable member and the module's public
exports. Bare glob imports are rejected; namespace imports must be spelled
`import * as name from "path"`. Imported public structs can be used across
modules, but private fields can only be constructed or read inside the defining
module.

Next work:

- Decide whether package-private visibility exists once package identity and a
  concrete use case are stable.

Acceptance:

- Visibility errors are precise and stable across selected and namespace imports.

## Phase 6: Hardening And Portability

Goal: make the implementation dependable enough for sustained development.

### 13. Diagnostics And Tooling

Current state: lexer and parser failures in package-loading CLI paths now report
`path:line:column: lex error` or `path:line:column: parse error`, including
`mo check` and `mo test` bad-source paths. Duplicate top-level function
semantic errors carry source locations through package loading and print as
`path:line:column: semantic error`. The first stable diagnostic codes are
`MO0001` for lex errors, `MO0002` for parse errors, and `MO1001` for duplicate
top-level symbols. Token dumps still keep byte spans for token inspection.

Next work:

- Extend source locations beyond duplicate top-level functions through HIR/IR
  for resolver, type, ownership, borrow, drop, and thread diagnostics.
- Extend structured error codes beyond the current lex, parse, and duplicate
  top-level semantic diagnostics.
- Keep `mo fmt`, `mo test`, metrics, and memory reports reliable.

Acceptance:

- New semantic checks include line/column diagnostics.
- CLI tests cover representative bad programs.

### 14. Runtime Portability

Current state: macOS/aarch64 is the executable target. Runtime configuration is
target-keyed, with explicit Linux x86_64/aarch64 GNU stubs for libc, pthread,
socket, time, and allocator symbols. Target-independent code now checks,
lowers, resolves, and type-checks under the Linux target in tests; native Linux
object emission is still pending.

Next work:

- Add Linux object emission/linking once the backend target boundary is ready.
- Separate platform tests from language tests.

Acceptance:

- macOS remains green.
- Linux target can at least check/compile target-independent code.

### 15. Performance And Load Regression

Current state: load tooling exists and a small Pokémon smoke has been recorded. Demo coverage now also includes `demos/sqlite`, which exercises manifest-distributed native static linking, and `demos/raytracer`, which exercises executable `Float64` arithmetic, borrowed structs, loops, file output, and idiomatic boolean/compound operators by writing a PPM image to `/tmp/mo_raytracer.ppm`.
The benchmark script now waits for the demo readiness marker before sending
traffic, avoiding startup races where clients hit the server before route
registration and async probes complete. The current Phase 15 smoke artifacts are
stored as `build/pokemon_bench_phase15_smoke.json`,
`build/pokemon_bench_phase15_smoke.csv`, and
`build/pokemon_bench_phase15_smoke_rss.csv`; the recorded run completed 24/24
requests with 20 HTTP 200 responses and 4 HTTP 201 responses. The threadpool
REST compile test now asserts a 2xx status class instead of only checking that
the response starts with `HTTP`.

Next work:

- Run repeatable load tests after networking/async/runtime changes.
- Add allocation/free/live/high-water runtime counters to the benchmark output;
  the script currently records latency, throughput, status mix, bytes read, and
  RSS samples.
- Add regression thresholds only after measurements stabilize.

Acceptance:

- Benchmark output is stored consistently.
- Regressions are visible before server behavior changes ship.

## Near-Term Queue

Work these in order unless a bug blocks the suite:

1. Complete public `String`/`Str` semantics and remove more internal `OwnedString` special casing.
2. Generalize recursive/path-sensitive drop glue for compound owned values.
3. Strengthen enum/pattern and `Result`/`Option` typing.
4. Establish the `alloc` package boundary.
5. Generalize `Vec<T>` enough to back richer route metadata and headers.
6. Implement real `Future<T>` representation and `async.block_on`.
7. Replace readiness-gated `async_tcp` helpers with future-returning TCP operations.
8. Convert `lib/express.App` fixed route slots to collection-backed route tables. Routes now use Vec-backed method/path/handler tables in both serial and threadpool serving paths; parameterized route metadata is still pending.

## Completed Prototype Milestones

The previous roadmap queue is complete at the prototype-slice level:

- `Str` and owned `String` bridge.
- General drop model foundations.
- `Box`, `Buffer`, `Vec`, `Map`, `Shared`.
- Runtime memory counters and load tooling.
- Typed unsafe pointers.
- Typed TCP listener/stream wrappers.
- Event loop abstraction.
- Readiness-gated async TCP helpers.
- Immediate executable async/await slice.
- `std/`, `core/`, and `lib/` import roots.
- Namespace aliases independent from filenames.
- Initial route-table metadata and middleware hook.
- Pokémon REST and threadpool demo paths.
