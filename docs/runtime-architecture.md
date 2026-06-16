# Runtime And Library Architecture

Mo should not hide library behavior in the compiler. The compiler provides the minimum primitives needed to implement the language safely; libraries provide the user-facing APIs in Mo.

The ownership model in [ownership-roadmap.md](ownership-roadmap.md) is controlling for memory safety: copy small values, move owned values, borrow with `&`/`Str`, clone explicitly, share with `Shared<T>`, drop automatically, and isolate raw memory in `unsafe`. Implementation sequencing lives in [roadmap.md](roadmap.md).

## Layering

### `core`

`core` is freestanding. It must not depend on libc, syscalls, files, sockets, threads, environment variables, or a process allocator.

Allowed contents:

- Primitive type definitions and traits/interfaces, including borrowed view types such as `Str`.
- `Option`, `Result`, `Never`, numeric types, `Bool`, `Char`, `Str`, raw pointer marker types.
- Minimal unsafe intrinsic declarations.
- Target-independent language support.

### `alloc`

`alloc` depends on an allocator contract, not an operating system.

Allowed contents:

- `Box`.
- `String`.
- `Buffer` / `StringBuilder`.
- `Vec`.
- `Map` only if it can be implemented without OS services.
- Slices and owned heap containers.

### `std`

`std` depends on `core` and `alloc`, and is the first OS-aware layer.

Allowed contents:

- Filesystem.
- TCP/UDP networking.
- Threads.
- Time.
- Environment/process APIs.
- Platform-specific implementations selected through target directives.

`std` may call C or syscalls internally for the macOS backend, but those calls must sit behind Mo APIs and target-gated modules. Public user code should not need raw C layouts for normal IO/networking.

### Userland Packages

Protocol and ecosystem libraries live outside `std`.

Examples:

- `serde`-style encoding/decoding.
- JSON parser/encoder.
- HTTP server framework.
- Express-like router.
- SSE helpers.
- Middleware.

The standard library may expose low-level HTTP building blocks only if they are needed as runtime infrastructure. High-level web frameworks remain userland.

## Compiler Intrinsic Policy

Compiler-known operations are allowed only when they are true language/runtime bricks.

Allowed examples:

- Allocation/deallocation hooks.
- Raw load/store operations inside `unsafe`.
- Backend ABI calls.
- Private string layout operations behind `core/unsafe` while `String` layout is still backend-owned.
- Function pointer lowering.
- Drop glue.
- Temporary ownership metadata such as IR-level `OwnedString` while the compiler still owns string layout.
- Platform target predicates.

Disallowed examples:

- JSON parsing/encoding.
- HTTP response formatting.
- Router behavior.
- Header maps.
- High-level string builders implemented in the compiler.
- Any protocol library behavior.

Every compiler-known operation must have:

1. A documented reason it cannot yet be written in Mo.
2. A narrow signature.
3. A migration path to Mo code once the required lower-level bricks exist.
4. Tests that exercise the public Mo API, not only the compiler path.

## Current Debt To Remove

These are temporary implementation shortcuts and should not become design precedent:

- `alloc/string.mo` now provides the first allocation-facing string boundary over `core/unsafe` wrappers for copy, concat, int/byte formatting, and free.
- `std/string.mo` provides the public `String.new`, `String.len`, `String.concat`, `String.from_int`, and `String.from_byte` APIs as Mo functions and delegates owning allocation work through `alloc/string`.
- `alloc/buffer.mo` owns the raw string-backed allocation, byte load/store, and free helpers used by `std/buffer.mo`. `std/buffer.mo` still provides the public fixed/growing buffer slice plus StringBuilder- and byte-buffer-named facades over that storage; long term this should become `alloc.Buffer` with explicit ownership transfer and allocator-contract semantics.
- `alloc/box.mo` owns Box cell allocation, cell load/store, string pointer storage, and cell free helpers used by `std/box.mo`.
- `alloc/vec.mo` owns Vec slot allocation, slot load/store, string pointer storage, legacy handler and typed request-handler function-pointer storage, element string free, and slot free helpers used by `std/vec.mo`. `std/vec.mo` exposes a narrow specialized `data<T>` accessor for current runtime table serialization, alongside the higher-level length/capacity/push/get/destroy facade.
- `alloc/map.mo` owns the current Vec-backed string/string map storage policy used by `std/map.mo`.
- The IR currently has a temporary `OwnedString` marker to distinguish heap-owned strings from borrowed/static string values for simple local drops. This prevents freeing literals while allowing automatic cleanup for concat/int/byte formatting results and simple wrappers. It is a bridge toward the public `Str`/owned `String` model, not the final ownership implementation.
- `raw_alloc`, `raw_free`, `raw_load8`, `raw_store8`, and `raw_write` still exist as backend intrinsics, but they are no longer prelude names. Current Mo code reaches them through `core/unsafe.mo`.
- `std/http.mo` parses the current benchmark request method/route/content-length/body into a typed `Request`, stores arbitrary exact-name request headers in Vec-backed string/string storage, keeps `read_body` as a lower-level compatibility helper for unconsumed fds, and builds typed status/body/content-type/header `Response` values before rendering/writing them. `lib/express.mo` consumes that typed request in serial and threadpool serving paths, matches registered routes by parsed method/path, passes `&http.Request` into middleware and route handlers, and writes typed `http.Response` values returned by route handlers. Long term, request/response building should support richer typed body collections and live in a lower-level HTTP package or userland HTTP library.
- `std/net.mo` still manually writes C socket structs with raw byte stores. Long term, platform modules should hide that behind typed Mo APIs.
- `std/fs.mo` currently calls fixed-ABI libc functions directly for macOS. This is intentional for now; variadic C calls such as `open(path, flags, mode)` must not be exposed as ordinary fixed-signature externs until the compiler has explicit varargs ABI support.
- `lib/json.mo`, `lib/pokemon.mo`, `lib/express.mo`, and `lib/pokemon_server.mo` are userland packages. They must not move into `std`; the standard library should provide bytes, strings, files, paths, process, networking, and eventually collections/buffers that make these packages natural to write.
- `lib/express.mo` must stay resource-agnostic. Resource-specific routes such as the Pokémon REST demo belong in userland composition modules like `lib/pokemon_server.mo`. Its current `server.get(...)` / `server.post(...)` API is a userland method-call wrapper over Vec-backed route and middleware tables, not compiler-known router behavior.
- `std/async.mo` currently wraps the pthread-backed task primitive. It is a bridge toward a real async executor; it does not yet implement future polling or `.await` scheduling.
- Runtime memory accounting now exists for backend-mediated heap operations. `core/unsafe.mo` exposes allocation count, free count, live bytes, and high-water bytes. Current counters use allocator-reported usable sizes on macOS, so they are suitable for regression checks but are not a final cross-platform allocator contract.

## Migration Order

1. Create `core` intrinsic module boundaries for allocation, raw memory, and ABI/syscall calls. Initial `core/unsafe.mo` boundary is done.
2. Add runtime allocation/free/live-byte/high-water instrumentation around allocator hooks. Initial macOS/backend-mediated instrumentation is done.
3. Add public `Str` as the borrowed string-view type and migrate read-only string APIs toward it.
4. Implement `alloc.String` on top of explicit layout and allocator hooks so public `String` means owned heap string.
5. Implement `alloc.Buffer` / `StringBuilder`. Initial `alloc/buffer.mo` storage helpers, the fixed/growing `std/buffer.mo` slice, and first-class public `buffer.StringBuilder` / `buffer.ByteBuffer` owner types over that storage are done; a full public `alloc.Buffer` ownership contract remains pending.
6. Replace the temporary IR-level `OwnedString` marker and `core/unsafe` string bricks with `alloc.String`/`alloc.Buffer` implementations once owned buffers exist.
7. Move raw memory operations out of prelude and into `core/unsafe`. Initial migration is done.
8. Add general `Drop`, `Box<T>`, then `Vec`, slices, and `Map` in Mo. Initial `alloc/box.mo` cell helpers, `alloc/vec.mo` slot helpers including legacy handler and typed request-handler function-pointer storage, `alloc/map.mo` string/string storage policy, and Vec-backed Express method/path/handler route tables are done; fully generic public Box/Vec/Map implementations remain pending.
9. Add `Shared<T>` and typed `RawPtr<T>`.
10. Refactor `std.net` to use typed internal platform structs.
11. Move JSON fully to userland, with a package name such as `json` or later `serde_json`.
12. Build the HTTP server library on top of `std.net`, `alloc`, and userland JSON.
