# Mo Ownership Model

This document defines the ownership and memory-safety model. It is not a
separate active roadmap. Implementation sequencing lives in
[roadmap.md](roadmap.md).

Mo should keep the written model smaller than Rust while retaining deterministic
cleanup, thread safety, and no garbage collector.

## User Model

```text
Copy small values.
Move owned values.
Borrow with & or Str.
Clone explicitly.
Share with Shared<T>.
Drop automatically.
Use unsafe for raw memory.
```

The compiler may use Rust-like internal machinery, but ordinary Mo code should
not need explicit lifetime parameters for common programs.

## Rules

1. Every resource has one owner unless it is explicitly shared.
2. Assigning or passing an owned value by value moves it.
3. `clone` creates another owned value when a type supports cloning.
4. `&T`, `&mut T`, and `Str` borrow without taking ownership.
5. Owned values are dropped automatically at the end of their live scope.
6. `Shared<T>` is the explicit shared-ownership mechanism.
7. Raw memory and raw pointers belong behind `unsafe` and typed unsafe APIs.

## Memory Categories

### Value

Small copied values:

```mo
Int
Bool
Byte
Int32
UInt32
```

Copying them does not affect ownership.

### Owned

Unique resource-owning values:

```mo
String
Buffer
TaskQueue4
TaskQueue4Int
Box<T>
Vec<T>
Map<K, V>
File
Socket
TcpListener
TcpStream
```

Owned values move by default and drop automatically when their ownership model
is implemented.

`Channel<T>` is currently a cloneable shared handle. Its lightweight wrapper can
be dropped, but destructive cleanup of shared internals remains explicit until
the channel internals get a distinct unique owner or shared-owned inner
resource.

### Borrowed

Temporary access to an owned or static value:

```mo
&T
&mut T
Str
```

Borrowed values do not own resources and are not dropped.

### Shared

Reference-counted shared ownership:

```mo
Shared<T>
```

`Shared<T>` should be thread-safe by default. A single-threaded shared owner can
be considered later only if it has a clear performance or ergonomics payoff.

### Unsafe

Raw memory must stay isolated:

```mo
unsafe {
    let ptr = core.alloc(64)
    core.free(ptr)
}
```

Target public unsafe types:

```mo
RawPtr<T>
NonNull<T>
```

Raw pointers must not be represented as ordinary `Int` in safe public APIs.

## String Model

Target public model:

```mo
Str     // borrowed UTF-8 string view
String  // owned heap string
```

Examples:

```mo
let view: Str = "Pikachu"
let owned = String.from("Pikachu")
```

Read-only functions should usually accept `Str`:

```mo
fn greet(name: Str) {
    print(name)
}
```

Owned `String` values should coerce to `Str` for read-only calls without moving.

Current implementation status: frontend type checking now keeps `Str` distinct
from owned `String`. String literals infer as `Str`; assigning or passing a
literal directly where an owned `String` is required is rejected; and owned
`String` values remain readable through `Str` parameters without moving.

## Receiver Model

Receiver type declares ownership behavior:

```mo
fn len(self: &String) -> Int
fn push(self: &mut Buffer, text: Str)
fn finish(self: Buffer) -> String
```

Surface calls stay simple:

```mo
let mut buf = Buffer.new()
buf.push("hello")
let text = buf.finish()
```

After a consuming call, using the consumed value is an error.

## Drop Model

Drop is deterministic. A type may implement `Drop` to provide custom cleanup:

```mo
interface Drop {
    fn drop(&self)
}
```

When a live value leaves scope, compiler drop glue should:

1. recursively drop owned fields,
2. call custom drop behavior where present,
3. release wrapper storage when appropriate,
4. avoid double-drops after moves or explicit ownership transfer.

The active roadmap tracks the remaining work to generalize this across all
compound expression and return paths.

## Thread Safety

Values moved into threads or async tasks must be safe to send. Current rules
reject borrowed references and raw pointers crossing thread/channel boundaries.

Long-term model:

```mo
interface Send
interface Sync
```

Compiler-derived `Send`/`Sync` should be structural for ordinary safe types and
explicit for unsafe/shared internals.

## Current Prototype Coverage

Implemented at the executable prototype level:

- public `Str`/`String` bridge,
- `String.from` and `String.clone`,
- local move/use-after-move checks,
- shared and mutable borrow checks,
- no direct local-reference returns,
- automatic drops for several owned resources,
- custom `Drop` recording and backend calls,
- `Box`, `Vec`, `Map`, `Shared` slices,
- typed unsafe pointer bridge,
- thread/channel rejection for borrows and raw pointers.

Remaining work is tracked in [roadmap.md](roadmap.md), primarily under core
semantics, allocation/collections, and async phases.
