# Mo Language Feature Design

Mo is a compiled systems programming language with Rust-grade safety, Go-grade readability, deterministic memory management, and a backend-independent compiler pipeline.

Working slogan:

> Safe as Rust, simple as Go, no GC.

This document describes the high-level feature set. The formal language rules live in [reference.md](reference.md).

## Goals

- Memory safety by default.
- No garbage collector.
- Deterministic destruction.
- Simple, readable syntax.
- Fast compilation.
- Strong standard library.
- Safe concurrency.
- Explicit unsafe boundary.
- Portable compiler pipeline with a Mo IR before LLVM, Cranelift, or other backends.

## Initial Platform Target

Mo targets macOS first.

The initial compiler, runtime, standard library, async executor, threadpool, networking, and web server target should only need to work on macOS. Cross-platform support is a design requirement, but not an implementation requirement for the first working system.

The language must still provide a compiler-supported way to write platform-dependent code. Platform selection should be explicit, typed, and checked by the compiler instead of handled with ad hoc build scripts.

Expected mechanisms:

- Target triples, starting with macOS targets such as `aarch64-apple-darwin` and `x86_64-apple-darwin`.
- Compiler directives for conditional compilation.
- Target-specific modules.
- Standard library platform backends hidden behind stable public APIs.
- Compile-time errors when no implementation exists for the active target.

Example:

```mo
@target(.macos) {
    fn page_size() -> Int {
        darwin.page_size()
    }
}

@target(.linux) {
    fn page_size() -> Int {
        linux.page_size()
    }
}
```

The first implementation only needs the macOS branch, but the language design should not make platform expansion painful later.

## First Target Program

The first serious end-to-end program should be a production-shaped web server:

- Async request handling.
- Threadpooled execution.
- HTTP routing.
- JSON request and response bodies.
- Server-sent events.
- File and socket IO.
- Graceful shutdown.
- Structured errors.

This target should guide early language and standard library choices. If a feature does not help build this server, the compiler, or the safety model, it should be postponed.

Example target shape:

```mo
import { Request, Response, Server } from "std/http"

struct Pokemon {
    id: Int
    name: String
    kind: String
    level: Int
}

async fn get_pokemon(req: Request) -> Result<Response, Error> {
    let pokemon = Pokemon { id: 25, name: "Pikachu", kind: "Electric", level: 5 }
    Response.json(pokemon)
}

async fn train_pokemon(req: Request) -> Result<Response, Error> {
    let pokemon = Pokemon { id: 25, name: "Pikachu", kind: "Electric", level: 6 }
    Response.json(pokemon)
}

async fn main() -> Result<(), Error> {
    Server.new()
        .workers(thread.cpu_count())
        .get("/pokemon", get_pokemon)
        .post("/pokemon", train_pokemon)
        .listen("127.0.0.1:3000")
        .await
}
```

## Non-Goals

Mo should not initially include:

- Garbage collection as the default memory model.
- Inheritance-based object orientation.
- Exceptions.
- Null references.
- User-visible lifetime annotations in ordinary code.
- A complex Rust-style trait system.
- Macro-heavy metaprogramming.
- Advanced type-level programming.

## Design Principles

### Safety Is Default

Safe Mo code should prevent:

- Use-after-free.
- Double free.
- Null dereference.
- Data races.
- Dangling references.
- Invalid enum states.
- Unchecked buffer access.

Unsafe behavior is only available inside explicit `unsafe` code.

### Ownership Exists, But Stays Quiet

Mo uses ownership, moves, borrows, and deterministic drops internally. The language should expose these concepts only when they matter.

Common code should look closer to Go than Rust:

```mo
import * as String from "std/string"

let name = String.new("Ada")
print(name)
save(name)
```

The function signatures decide whether a value is borrowed or moved:

```mo
fn print(name: &String)
fn save(name: String)
```

### Mutation Is Visible

Shared reads should be lightweight. Mutation should be visible at the binding and call site.

```mo
let mut user = User.new("Ada")
user.rename("Grace")
clear(mut user)
```

### Small Interfaces

Interfaces should be simple behavioral contracts. Prefer small interfaces like `Reader`, `Writer`, `Display`, and `Hasher`.

Mo uses explicit interface conformance on the type declaration for clarity:

```mo
interface Writer {
    fn write(&mut self, bytes: Slice<Byte>) -> Result<Int, Error>
}

struct File: Writer {
    fd: Int

    fn write(&mut self, bytes: Slice<Byte>) -> Result<Int, Error> {
        ...
    }
}
```

## Core Language Features

### Bindings

- `let` creates immutable bindings.
- `let mut` creates mutable bindings.
- Shadowing is allowed.
- Mutation requires a mutable binding.

```mo
let x = 1
let mut y = 2
y = y + 1
```

### Primitive Types

Core scalar types:

- `Bool`
- `Byte`
- `Int`, `Int8`, `Int16`, `Int32`, `Int64`
- `UInt`, `UInt8`, `UInt16`, `UInt32`, `UInt64`
- `Float32`, `Float64`
- `Char`
- `Unit`

`Int` and `UInt` are pointer-sized signed and unsigned integers.

Numeric and byte helper APIs are in std modules:

```mo
import * as int from "std/int"
import * as bytes from "std/bytes"

let id = int.parse_decimal("42")
let digit = bytes.digit_value(55)
```

### Compound Types

- Tuples.
- Fixed arrays: `Array<T, N>`.
- Slices: `Slice<T>`.
- Structs.
- Enums.
- Function types.
- References.
- Raw pointers in unsafe code.

### Functions

```mo
fn add(a: Int, b: Int) -> Int {
    a + b
}
```

Functions are expression-oriented. The last expression in a block is the return value when no `return` is used.

### Anonymous Functions And Closures

Mo has anonymous functions.

```mo
let add = fn(a: Int, b: Int) -> Int {
    a + b
}
```

Anonymous functions may capture values from their surrounding scope.

```mo
let prefix = "user:"
let label = fn(id: Int) -> String {
    prefix + id.to_string()
}
```

Capture rules follow the ownership model:

- Shared captures borrow by default.
- Mutable captures require mutable access.
- Owned captures use `move fn`.
- A closure cannot outlive borrowed captures.
- A closure moved into a thread must own its captures and implement `Send`.

Thread example:

```mo
import * as String from "std/string"

let message = String.new("hello")

thread.spawn(move fn() {
    print(message)
})
```

Async closures use `async fn`.

```mo
let handler = async fn(req: Request) -> Result<Response, Error> {
    Response.text("ok")
}
```

Closures are required for the web server target, callbacks, iterators, thread spawning, and async task composition.

### Structs

```mo
struct User {
    id: Int
    name: String
}
```

Struct construction:

```mo
let user = User {
    id: 1
    name: "Ada"
}
```

### Methods

Methods live inside the type declaration.

```mo
struct User {
    id: Int
    name: String

    fn new(id: Int, name: String) -> User {
        User { id, name }
    }

    fn name(&self) -> &String {
        &self.name
    }

    fn rename(&mut self, name: String) {
        self.name = name
    }

    fn into_name(self) -> String {
        self.name
    }
}
```

Receiver modes:

- `&self`: shared borrow.
- `&mut self`: mutable borrow.
- `self`: consume the value.

Current executable support includes method-call syntax for side-effecting calls where a top-level/imported function takes the receiver as its first parameter. For example, `server.get(path, handler)` registers a route handler by mutating the server app. Function-valued struct fields remain callable, so `app.get_pokemon(client, context)` calls the stored handler rather than method-dispatching on `get_pokemon`.

### Enums

Enums are algebraic data types.

```mo
enum Option<T> {
    Some(T)
    None
}

enum Result<T, E> {
    Ok(T)
    Err(E)
}
```

### Pattern Matching

Pattern matching is exhaustive by default.

```mo
match result {
    Ok(value) => print(value)
    Err(error) => log(error)
}
```

Supported patterns:

- Literal patterns.
- Identifier binding patterns.
- Wildcard pattern `_`.
- Tuple patterns.
- Struct patterns.
- Enum variant patterns.
- Guarded patterns.

### Ownership And Borrowing

Rules:

- Every non-copy value has exactly one owner.
- Passing a non-copy value by value moves it.
- A moved value cannot be used again.
- Values are dropped at the end of their scope.
- Shared borrows permit reading.
- Mutable borrows permit mutation.
- A value may have many shared borrows or one mutable borrow.
- References cannot outlive their referents.
- Lifetime relationships are inferred in ordinary code.

### No Null

There are no null safe references.

Optional values use `Option<T>`:

```mo
fn find_user(id: Int) -> Option<User>
```

### Error Handling

Mo uses `Result<T, E>` for recoverable errors.

```mo
fn load(path: String) -> Result<String, IOError> {
    let file = File.open(path)?
    file.read_to_string()
}
```

There are no exceptions.

Panics may exist for unrecoverable programmer errors, but should not be used for normal error handling.

### Interfaces

Interfaces describe required methods.

```mo
interface Display {
    fn display(&self) -> String
}
```

Types explicitly declare interface conformance:

```mo
struct User: Display {
    name: String

    fn display(&self) -> String {
        self.name.clone()
    }
}
```

Generic use:

```mo
fn print(value: Display) {
    io.print(value.display())
}
```

Named generic use:

```mo
fn print_all<T: Display>(items: Slice<T>) {
    for item in items {
        print(item)
    }
}
```

Dynamic dispatch:

```mo
fn log_to(writer: &mut dyn Writer, msg: String) -> Result<(), Error> {
    writer.write(msg.bytes())
}
```

### Generics

Mo supports generic functions, structs, enums, and interfaces.

```mo
struct Vec<T> {
    ...
}

fn first<T>(items: Slice<T>) -> Option<&T> {
    ...
}
```

Initial implementation should use monomorphization.

### Modules

Modules organize code.

```mo
module user

pub struct User {
    pub id: Int
    name: String
}
```

Imports:

```mo
import * as fs from "std/fs"
import { Reader, Writer } from "std/io"
```

Items are private by default. `pub` makes an item public.

### Unsafe

Unsafe code is explicit:

```mo
unsafe {
    let ptr = alloc(size)
    *ptr = 10
}
```

Unsafe permits:

- Raw pointer dereference.
- FFI calls.
- Manual allocation APIs.
- Unchecked casts.
- Unsafe interface implementations.

Unsafe does not disable all type checking.

### C FFI

```mo
extern "C" {
    fn puts(s: *const Byte) -> Int32
}

@repr(.c)
struct Point {
    x: Float64
    y: Float64
}
```

## Concurrency

### Threads

Threads are provided by the standard library. The type system prevents data races in safe code.

```mo
import * as thread from "std/thread"

fn main() {
    let handle = thread.spawn(move fn() {
        print("hello")
    })

    handle.join()
}
```

Rules:

- Values moved into a thread must implement `Send`.
- Shared values must be immutable or protected by synchronization.
- Borrowed stack references cannot escape into spawned threads.
- Shared mutable state uses `Mutex<T>`, `RwLock<T>`, atomics, channels, or other safe synchronization types. The executable subset currently provides `std/sync.Mutex`, `std/sync.RwLock`, mutex-backed `std/atomic.AtomicInt`, and blocking `std/channel.Channel<Bool>` / `std/channel.Channel<Int>` / `std/channel.Channel<String>` / `std/channel.Channel<fn() -> ()>` with inferred specialization for annotated channel locals.

### Async And Await

Mo supports `async fn` and `.await`.

```mo
async fn fetch_user(id: Int) -> Result<User, Error> {
    let response = http.get("/users/{id}").await?
    response.json<User>().await
}
```

Design:

- `async fn` returns a future value.
- `.await` can suspend the current async computation.
- Async functions lower to state machines in Mo IR.
- Futures are ordinary values and are not garbage-collected.
- Borrows across `.await` are checked.
- The core language is runtime-neutral.
- The standard library provides a default executor.

Example:

```mo
import * as async from "std/async"

async fn main() -> Result<(), Error> {
    let task = async.spawn(fetch_user(1))
    let user = task.await?
    print(user.name)
    Ok(())
}
```

## Standard Library

The standard library is part of the language design.

Suggested module layout:

```text
std.core
std.mem
std.alloc
std.collections
std.io
std.fs
std.path
std.process
std.thread
std.sync
std.async
std.time
std.net
std.http
std.sse
std.test
std.math
std.log
std.c
```

### Core Types

- `Option<T>`
- `Result<T, E>`
- `String`
- `Slice<T>`
- `Array<T, N>`
- `Box<T>`
- `Vec<T>`

### Core Interfaces

- `Copy`
- `Clone`
- `Drop`
- `Display`
- `Debug`
- `Eq`
- `Ord`
- `Hash`
- `Iterator`
- `Send`
- `Sync`

### Collections

- `Vec<T>`
- `Map<K, V>`
- `Set<T>`
- `Deque<T>`
- first-class `StringBuilder` and `ByteBuffer` owner types over current buffer storage
- borrowed `ByteSlice` bridge over string backing with `expr[index]` indexing

### Memory

- `Box<T>`
- `Rc<T>`
- `Weak<T>`
- `Arc<T>`
- arenas, later
- allocator APIs, later

Reference counting is explicit library functionality, not hidden GC.

### IO And Filesystem

```mo
let text = fs.read_text("config.mo")?
fs.write_text("out.txt", text)?
```

### Synchronization

- `Mutex<T>`; current executable subset: `std/sync.Mutex` handle.
- `RwLock<T>`; current executable subset: `std/sync.RwLock` handle.
- `Once<T>`
- `Atomic<T>`
- `Channel<T>`; current executable subset: blocking `std/channel.Channel<Bool>`, `std/channel.Channel<Int>`, `std/channel.Channel<String>`, and `std/channel.Channel<fn() -> ()>` with inferred specialization for annotated channel locals and channel arguments.

### Networking And Web

`std.io` provides low-level descriptor IO used by early runtime and server work.

Current executable IO surface:

```mo
import * as io from "std/io"

fn write_log(fd: Int) -> Int {
    return io.write_fd(fd, "ready\n")
}
```

`std.net` provides TCP, UDP, addresses, listeners, and streams.

Current executable networking surface:

```mo
import * as net from "std/net"

fn main() -> Int {
    let fd = net.tcp_listen_ephemeral(16)
    if fd > 0 {
        let port = net.listener_port(fd)
        net.wait_readable(fd)
        net.set_nonblocking(fd)
        net.close_fd(fd)
        return 0
    }
    return 1
}
```

`std.http` provides enough HTTP support for the current Pokémon REST benchmark:

- Request and response types.
- Headers.
- Current executable surface: `http.read_request(fd)` parses the benchmark method, path, route ID, `Content-Length`, body, and arbitrary exact-name request headers into a typed `Request`; `http.Response` owns status, body, and content type; `http.render(response)` and `http.write_response(fd, response)` render typed responses with computed `Content-Length`; and compatibility helpers such as `http.request_route(fd)`, `http.write_json(fd, body)`, and `http.write_not_found(fd)` remain available for lower-level callers and older tests.
- Generic request parsing and status-code helpers beyond the benchmark routes.
- Route tables and dynamic router data structures.
- Body streaming.
- Async server.
- Threadpool configuration.
- Graceful shutdown hooks.

Userland JSON starts in `lib/json.mo`. It provides string encoding, integer encoding, and small field parsers used by userland resource packages.

Userland resource storage starts in `lib/pokemon.mo`. It composes `std/fs` with `lib/json.mo` and is intentionally outside `std`.

Userland server composition is split by responsibility:

- `lib/express.mo` is reusable HTTP/TCP plumbing over `std/net` and `std/http`: listen, accept, parse typed requests, route by method/path tables, pass `&http.Request` to middleware/handlers, write typed `http.Response` handler returns, close clients, and register route handlers with `server.get(...)` / `server.post(...)` method-call syntax.
- `lib/pokemon_server.mo` composes `lib/express.mo` with `lib/pokemon.mo` to implement `GET /pokemon`, `POST /pokemon`, `GET /health`, fixed-count serving, and the current fixed four-worker smoke path.
- `examples/compile/pokemon_rest_api.mo`, `examples/compile/pokemon_threadpool_rest_api.mo`, and `examples/demo/pokemon_server.mo` are applications that compose `express`, `pokemon_server`, and the Pokémon resource package.

JSON parsing, encoding, and typed serialization belong in userland packages such as `json` or a later `serde_json`.

`std.sse` provides server-sent event streams on top of HTTP response bodies.

### Testing

Mo has a built-in test declaration and a `mo test` command.

```mo
test "user can be renamed" {
    let mut user = User.new(1, "Ada")
    user.rename("Grace")
    assert(user.name == "Grace")
}
```

Executable tests run with:

```text
mo test examples/compile
```

The current compiler builds each test body as a temporary native executable and
uses exit code `0` as pass.
`assert(Bool)` prints `assertion failed` and fails the test with exit code `1`
when the condition is false.

## Compiler Architecture

Pipeline:

```text
Source
  -> Lexer
  -> Parser
  -> AST
  -> Name resolution
  -> Type checking
  -> Ownership and borrow checking
  -> Mo IR
  -> Optimization passes
  -> Backend lowering
      -> LLVM
      -> Cranelift
      -> C
      -> WASM
```

Platform selection is resolved before or during name resolution. Disabled `@target` blocks are not type-checked for the active target, but each target configuration must be checkable independently.

Mo IR should be:

- Typed.
- SSA-based or SSA-friendly.
- Explicit about control flow.
- Explicit about ownership and drops.
- Backend-independent.
- Lower-level than AST.
- Higher-level than LLVM IR.

Example:

```text
fn add(Int a, Int b) -> Int {
block0:
    %0 = add_int a, b
    return %0
}
```

Ownership-aware example:

```text
%user = construct User(...)
%name = field_borrow %user.name
call print(%name)
drop %user
return
```

## Milestones

### M0: Tiny Core

- Lexer and parser.
- Variables.
- Primitive types.
- Functions.
- Structs.
- Methods.
- Simple modules.
- Mo IR.
- One backend.
- macOS target support.
- `@target` filtering.
- `test` item parsing.
- `mo test` native execution for backend-supported test bodies.

### M1: Real Language

- Enums.
- Pattern matching.
- Ownership and moves.
- Borrow checking.
- Drops.
- `Option`.
- `Result`.
- Basic standard library.

### M2: Usable Systems Language

- Generics.
- Interfaces.
- C FFI.
- Unsafe blocks.
- Collections.
- Filesystem and IO.
- Test body execution.
- JSON basics.

### M3: Concurrency

- Threads.
- `Send` and `Sync`.
- Mutexes.
- Atomics.
- Channels.
- `Arc<T>`.
- Threadpool.
- macOS networking and thread primitives.

### M4: Async

- `async fn`.
- `.await`.
- Future lowering.
- Default executor.
- Async timers.
- Async networking.
- Async HTTP server.
- SSE streaming.

## Open Design Questions

- Should read-only borrowing be implicit at call sites?
- Should mutable borrowing use `mut value` at call sites?
- How much lifetime syntax should exist for advanced library authors?
- Should interface conformance ever be implicit for local/private types?
- Should `go { ... }` be syntax, or should threads only be spawned through `std.thread.spawn`?
- Should async be included in v1, or implemented after threads?
- Should `String` be in `std.core` or imported from `std.text`?
- Should integer overflow trap, wrap, or be mode-dependent?
- What exact `@target` predicate grammar should platform-dependent code use?
- Should target-specific files be selected by naming convention, module attributes, or package manifest rules?
