# Mo Language Reference

This document is the working reference for Mo. It describes language syntax and semantics at a level intended to guide implementation.

Mo is a statically typed, compiled systems language with memory safety, no garbage collector, deterministic destruction, interfaces, pattern matching, threads, async/await, and a backend-independent intermediate representation.

The ownership model is intentionally small: copy small values, move owned values, borrow with `&` or `Str`, clone explicitly, share with `Shared<T>`, drop automatically, and isolate raw memory in `unsafe`. See [ownership-roadmap.md](ownership-roadmap.md). Implementation sequencing lives in [roadmap.md](roadmap.md).

## 0. First Target Program

The first complete language demonstration should be an async, threadpooled web server.

It must exercise:

- Async functions and `.await`.
- Threadpool execution.
- TCP networking.
- HTTP request and response handling.
- JSON parsing and encoding.
- Server-sent events.
- Structured error handling.
- Safe sharing between worker threads.

The first implementation target is macOS only. The language and compiler must still support target-aware code so the standard library can grow to other platforms later.

Representative program:

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

## 1. Lexical Structure

### 1.1 Source Files

Mo source files use UTF-8 text.

The default file extension is:

```text
.mo
```

### 1.2 Comments

Line comments begin with `//`.

```mo
// This is a comment.
```

Block comments begin with `/*` and end with `*/`.

```mo
/* This is a block comment. */
```

Nested block comments are allowed.

### 1.3 Compiler Directives

Compiler directives begin with `@` and attach compiler-level instructions to items, fields, statements, modules, or blocks.

```mo
@target(.macos) {
    fn current_platform() -> String {
        "macos"
    }
}
```

Initial required directives:

- `@target(...)`
- `@repr(...)`

The exact full directive grammar is deferred.

### 1.4 Identifiers

Identifiers begin with a letter or `_`, followed by letters, digits, or `_`.

```mo
name
user_id
_temporary
```

Identifiers are case-sensitive.

### 1.5 Keywords

Reserved keywords:

```text
async
await
break
const
continue
else
enum
extern
false
fn
for
if
interface
let
match
module
mut
pub
return
self
static
struct
test
true
type
unsafe
use
while
```

### 1.6 Literals

Boolean literals:

```mo
true
false
```

Integer literals:

```mo
0
42
1_000_000
0xff
0b1010
```

Floating-point literals:

```mo
3.14
1.0
1e9
```

String literals:

```mo
"hello"
```

String literals have the borrowed string-view type `Str`. `Str` is ABI-compatible with the current string pointer representation and does not own heap memory. Owned heap strings use `String`; use `String.from`, `String.new`, `String.clone`, `String.concat`, or another owning producer when an API requires `String`. Passing `String` by value transfers ownership to the callee; read-only APIs should take `&Str`.

Character literals:

```mo
'a'
```

## 2. Program Structure

### 2.1 Modules

A source file may declare its module:

```mo
module app.user
```

If omitted, the module is inferred from the package layout.

### 2.2 Items

Top-level items include:

- `fn`
- `struct`
- `enum`
- `interface`
- `type`
- `const`
- `static`
- `extern`
- `test`

### 2.3 Visibility

Items are private by default.

`pub` makes an item visible outside its module.

```mo
pub fn open(path: String) -> Result<File, IOError>
```

Struct fields are private by default:

```mo
pub struct User {
    pub id: Int
    name: String
}
```

Code in the defining module can construct and read every field. Code in other
modules can construct or read only fields marked `pub`; attempts to use private
fields report ``field `name` on `User` is private``.

### 2.4 Imports

```mo
import * as fs from "std/fs"
import { Reader, Writer } from "std/io"
import { User } from "./user"
```

Imports are relative to the file that contains the import when the path starts
with `./` or `../`, matching JavaScript-style module resolution.

```mo
import { add, Point } from "./math"
import * as math from "./math"
```

Current package roots:

```mo
import * as fs from "std/fs"
import * as core from "core/unsafe"
import * as alloc_string from "alloc/string"
import * as pokemon from "lib/pokemon"
```

`std/` resolves to the standard library, `core/` resolves to unsafe/core
building blocks, `alloc/` resolves to allocation internals, and `lib/` resolves
to the repository userland package root.

Reusable userland libraries should live in subdirectories under `lib/` when
they need their own manifest, tests, fixtures, or native assets. For example,
`lib/toml/` contains `mo.toml`, `src/toml.mo`, and `test/toml_test.mo`; its
manifest exposes a `test` script that runs the package-local tests from the
repository toolchain.

Mo also discovers the nearest ancestor `mo.toml` for an entry file. The current
manifest format supports package identity and local dependency roots:

```toml
[package]
name = "app"
root = "."

[dependencies]
math = "packages/math"

[target.macos.dependencies]
darwin_math = "platform/macos/math"
```

With that manifest, imports beginning with the dependency name resolve relative
to the configured root:

```mo
import { answer } from "math/answer"
```

Only `pub` items can be selected from another module with brace imports.
Namespace imports use `import * as name from "path"` and expose public members
through member access, so calls such as `math.add()` or `server.start()`
resolve through the imported module namespace. Private members remain
inaccessible through namespace aliases.

Visibility diagnostics name the requested missing or private item, the target
module, and the currently available public exports when the module can be
loaded. Private selected imports also include guidance to add `pub`. Private
names accessed through namespace imports report the unavailable member and the
module's available public exports.

The namespace alias does not need to match the filename:

```mo
import * as encoder from "lib/json"

let field = encoder.field_int("level", 42)
```

The current compiler carries first-class module IDs through HIR and IR.
Cranelift object symbols are module-qualified except for the exported platform
`main`.

## 3. Types

### 3.1 Primitive Types

```text
Bool
Byte
Int
Int8
Int16
Int32
Int64
UInt
UInt8
UInt16
UInt32
UInt64
Float32
Float64
Char
Unit
Never
```

`Unit` has one value: `()`.

`Never` represents computations that do not return. `return`, `break`, and `continue` have `Never` type internally, which lets branch expressions type-check when one branch produces a value and another branch exits.

Primitive numeric and byte types are core language types. Helper operations
live in standard modules instead of the prelude:

```mo
import * as int from "std/int"
import * as bytes from "std/bytes"
import * as float from "std/float"

let port = int.parse_decimal("3000")
let digit = bytes.digit_value(55)
let channel = float.to_int(float.clamp(0.75, 0.0, 1.0) * 255.0)
```

### 3.2 Struct Types

```mo
struct Point {
    x: Float64
    y: Float64
}
```

Struct values are created with field initializers:

```mo
let p = Point { x: 1.0, y: 2.0 }
```

Field shorthand is allowed:

```mo
let x = 1.0
let y = 2.0
let p = Point { x, y }
```

### 3.3 Tuple Types

```mo
let pair: (Int, String) = (1, "one")
```

### 3.4 Array Types

Fixed arrays have a compile-time length:

```mo
let bytes: Array<Byte, 4> = [1, 2, 3, 4]
```

### 3.5 Slice Types

A slice is a borrowed view into contiguous elements.

```mo
fn sum(values: Slice<Int>) -> Int
```

Mutable slices are written:

```mo
Slice<mut Int>
```

Current executable coverage is the `std/slice.ByteSlice` bridge:

```mo
import * as slice from "std/slice"

let whole = slice.from_str("abcd")
let mid = slice.subslice(whole, 1, 2)
let byte = mid[0]
```

`ByteSlice` supports `from_str`, `subslice`, `len`, `is_empty`, and bounded
`get`, returning `-1` for out-of-range indexes. `expr[index]` works for
`ByteSlice` and lowers to the same bounded `slice.get` path. It stores borrowed
`Str` backing through the lowered borrowed-string field type, so slicing a
string view does not clone the backing bytes. General first-class `Slice<T>`
lowering and mutable indexing remain active roadmap work.

### 3.6 Enum Types

```mo
enum Message {
    Quit
    Move { x: Int, y: Int }
    Write(String)
}
```

Enums may be generic:

```mo
enum Option<T> {
    Some(T)
    None
}
```

### 3.7 Function Types

```mo
fn(Int, Int) -> Int
```

Async function type:

```mo
async fn(Request) -> Result<Response, Error>
```

Function values may be named functions, anonymous functions, or closures.

### 3.8 Reference Types

Shared reference:

```mo
&T
```

Mutable reference:

```mo
&mut T
```

References are non-null and borrow-owned storage.

### 3.9 Raw Pointer Types

Raw pointers are unsafe:

```mo
*const T
*mut T
```

Raw pointers may be null and may dangle. Dereferencing a raw pointer requires `unsafe`.

The current executable byte-pointer bridge is in `core/unsafe`:

```mo
import * as core from "core/unsafe"

let ptr: *mut Byte = core.alloc_ptr(2)
core.store8_ptr(ptr, 0, 42)
let value = core.load8_ptr(ptr, 0)
core.free_ptr(ptr)
```

Raw pointer values are pointer-sized backend values, but the type checker keeps
them distinct from ordinary integers and bytes. Raw pointers are not `Send`.

### 3.10 Interface Object Types

Dynamic interface object:

```mo
dyn Writer
```

Dynamic interface objects are usually used behind references or owning pointers:

```mo
&mut dyn Writer
Box<dyn Writer>
```

## 4. Bindings And Variables

### 4.1 Immutable Bindings

```mo
let x = 1
```

Immutable bindings cannot be assigned after initialization.

### 4.2 Mutable Bindings

```mo
let mut x = 1
x = 2
```

### 4.3 Type Annotations

```mo
let x: Int32 = 1
```

Type annotations are optional when the compiler can infer the type.

### 4.4 Shadowing

Shadowing is allowed:

```mo
let value = "42"
let value = parse_int(value)?
```

## 5. Functions And Methods

### 5.1 Functions

```mo
fn add(a: Int, b: Int) -> Int {
    a + b
}
```

If the return type is omitted, it defaults to `Unit`.

```mo
fn log(msg: String) {
    print(msg)
}
```

### 5.2 Return

```mo
return value
return
```

A block's final expression is returned when no semicolon or explicit terminator is used.

### 5.3 Methods

Methods are declared inside the type body.

```mo
struct User {
    name: String

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
- `self`: move/consume.

### 5.4 Associated Functions

Functions in a type body without a receiver are associated functions.

```mo
struct User {
    name: String

    fn new(name: String) -> User {
        User { name }
    }
}

let user = User.new("Ada")
```

### 5.5 Anonymous Functions

Anonymous functions are function expressions.

```mo
let add = fn(a: Int, b: Int) -> Int {
    a + b
}
```

When the return type can be inferred, it may be omitted.

```mo
let is_empty = fn(s: &String) {
    s.len() == 0
}
```

### 5.6 Closures

An anonymous function that captures values from its surrounding scope is a closure.

```mo
let prefix = "user:"

let label = fn(id: Int) -> String {
    prefix + id.to_string()
}
```

Capture mode is inferred from use:

- Read-only use captures by shared borrow.
- Mutation captures by mutable borrow.
- `move fn` captures by value.

```mo
import * as String from "std/string"

let message = String.new("hello")

let print_message = move fn() {
    print(message)
}
```

A closure cannot outlive borrowed captures.

### 5.7 Async Anonymous Functions

Async anonymous functions use `async fn`.

```mo
let handler = async fn(req: Request) -> Result<Response, Error> {
    Response.text("ok")
}
```

Async closures may capture values. Captures held across `.await` are checked using the same borrow rules as async functions.

```mo
import * as String from "std/string"

let name = String.new("Ada")

let task = async fn() -> Result<(), Error> {
    print(name)
    async.sleep(1.sec).await
    Ok(())
}
```

Async closures lower to future values.

## 6. Expressions

### 6.1 Blocks

```mo
{
    let x = 1
    x + 1
}
```

Blocks create scopes and may evaluate to a value.

### 6.2 Operators

Mo supports equality and ordered comparisons:

```mo
a == b
a != b
a < b
a <= b
a > b
a >= b
```

Boolean operators:

```mo
!flag
a && b
a || b
```

Current `&&` and `||` lowering evaluates both operands before combining the
boolean results. They are boolean operators, not short-circuit control-flow
operators yet.

Integer, `Float64`, and arithmetic operators:

```mo
a + b
a - b
a * b
a / b
a % b
a & b
a | b
a ^ b
a << b
a >> b
```

`Float64` arithmetic lowers to native floating-point operations for `+`, `-`,
`*`, `/`, unary negation, equality, and ordered comparisons. Mixed `Int` and
`Float64` arithmetic converts the integer operand to `Float64` in IR.
`Float32` is currently a parsed core type name, but executable arithmetic is
implemented for `Float64`.

Compound assignment is available for mutable locals:

```mo
x += 1
x -= 1
x *= 2
x /= 2
x %= 3
x &= mask
x |= flag
x <<= 1
x >>= 1
```

### 6.3 If Expressions

```mo
let max = if a > b { a } else { b }
```

When used as an expression, all branches must produce compatible types.

### 6.3 While Loops

```mo
while condition {
    step()
}
```

### 6.4 For Loops

```mo
for item in items {
    print(item)
}
```

`for` loops use the `Iterator` interface.

### 6.5 Match Expressions

```mo
match value {
    Some(x) => x
    None => 0
}
```

Matches are exhaustive unless a wildcard arm exists. Positional enum payload bindings are typed from the matched enum value, so `Some(x)` in a match over `Option<Int>` binds `x` as `Int`.

Enum constructors check positional payload arity and payload types. Generic constructor payloads are checked when an expected enum type is available from an annotation, return, assignment, struct field, or call parameter. When there is no expected type, direct generic payloads are inferred from constructor arguments where possible; for example, `Some(41)` infers an `Option<Int>` value for type checking.

### 6.6 Break And Continue

```mo
break
continue
```

Loop labels may be added later.

### 6.7 Method Calls

```mo
user.rename("Grace")
```

Method-call receiver borrowing is inferred from the method receiver.

Current executable support resolves method calls through the receiver type. Lookup checks type-body methods, interface methods, and functions whose first parameter is compatible with the receiver. This enables userland APIs such as:

```mo
let mut server = express.with_backlog(128)
server.get("/pokemon", get_pokemon)
server.post("/pokemon", post_pokemon)
```

Function-valued struct fields remain callable and take precedence over method lookup:

```mo
app.get_pokemon(client, context)
```

Calls on known receiver types report an unknown-method diagnostic when no receiver-compatible method exists. Calls on imported reference-only types may remain unknown until their package surface is loaded.

### 6.8 Error Propagation

The `?` operator propagates `Err` or `None`.

```mo
let file = File.open(path)?
```

For `Result<T, E>`, `?` unwraps `Ok(T)` or returns `Err(E)`.

For `Option<T>`, `?` unwraps `Some(T)` or returns `None`.

The operand must be `Result<T, E>` or `Option<T>`. A `Result` `?` expression must be inside a function or closure returning `Result<_, E>` with a compatible error type. An `Option` `?` expression must be inside a function or closure returning `Option<_>`.

## 7. Patterns

### 7.1 Wildcard Pattern

```mo
_
```

### 7.2 Binding Pattern

```mo
name
```

### 7.3 Literal Pattern

```mo
0
"quit"
true
```

### 7.4 Tuple Pattern

```mo
let (id, name) = user_row
```

### 7.5 Struct Pattern

```mo
match user {
    User { id, name } => ...
}
```

### 7.6 Enum Pattern

```mo
match value {
    Some(x) => x
    None => default
}
```

### 7.7 Guards

```mo
match value {
    Some(x) if x > 0 => x
    _ => 0
}
```

## 8. Ownership And Borrowing

### 8.1 Ownership

Each non-copy value has one owner.

When the owner goes out of scope, the value is dropped.

```mo
import * as String from "std/string"

{
    let s = String.new("hello")
} // s is dropped here
```

`String.concat(a, b)` returns a string containing `a` followed by `b`.

```mo
import * as String from "std/string"

let message = String.concat("hello, ", "world")
```

`String.from_int(value)` returns a decimal string for an integer value.

```mo
import * as String from "std/string"

let size = String.from_int(42)
```

`String.clone(value)` creates a distinct owned copy while leaving the source usable.

```mo
import * as String from "std/string"

let original = String.from("hello")
let copied = String.clone(original)
```

`alloc/string` is the lower allocation-facing string boundary. It exposes
`copy`, `concat`, `from_int`, `from_byte`, and `free` for stdlib/internal code;
most user code should continue to import `std/string`.

Ownership contract: `copy`, `concat`, `from_int`, and `from_byte` return owned
`String` values. Callers must either return/store them in an owning public type
or eventually free them through the owning std wrapper/drop path. `free` releases
the owned string storage exactly once and is not a borrowing operation.

`alloc/buffer` is the lower storage boundary used by `std/buffer`. It exposes
raw string-backed `allocate`, `load`, `store`, and `free` helpers for
stdlib/internal buffer code; most user code should continue to import
`std/buffer`.

Ownership contract: `allocate` returns owned string-backed storage for a buffer.
`load` borrows that storage, `store` mutates it in place, and `free` consumes the
buffer storage obligation exactly once. `std/buffer` owns the public lifecycle.

`alloc/box` is the lower cell boundary used by `std/box`. It exposes cell
allocation, integer load/store, string pointer store/load, and cell free helpers
for stdlib/internal Box code; most user code should continue to import
`std/box`.

Ownership contract: `allocate_cell` returns raw owned cell storage. `store_*`
writes payload bits into the cell, `load_string` reconstructs an owned string
handle from a stored pointer, and `free_cell` releases only the cell storage, not
any payload ownership that has been moved out.

`alloc/vec` is the lower slot boundary used by `std/vec`. It exposes slot
allocation, integer slot load/store, string pointer store/load, legacy handler
and typed request-handler function-pointer store/load, string element free, and
slot free helpers for stdlib/internal Vec code; most user code should continue
to import `std/vec`.

Ownership contract: `allocate_slots` returns owned slot storage. Slot load/store
helpers only move pointer-sized payload representations; element ownership is
owned by the `std/vec` lifecycle. `free_string_at` releases one owned string
element, and `free_slots` releases only the slot buffer.

`alloc/map` is the lower storage-policy boundary used by `std/map`. It exposes
the current Vec-backed string/string key-value helpers for stdlib/internal Map
code; most user code should continue to import `std/map`.

Ownership contract: `new_string_keys` and `new_string_values` return owned Vec
storage. `put_string_string` consumes the key and value strings into those Vecs,
`get_string_string` returns a fresh owned string copy of the stored value, and
`destroy_string_string` releases the paired key/value storage exactly once. The
public `std/map.destroy_string_string` wrapper is treated as an explicit
consuming cleanup, so callers can destroy a map without an automatic second
cleanup at scope exit.

### 8.2 Moves

Passing or assigning a non-copy value by value moves ownership.

```mo
import * as String from "std/string"

let a = String.new("hello")
let b = a
// a cannot be used here
```

### 8.3 Copy Types

Types implementing `Copy` are copied instead of moved.

Primitive integers, floats, booleans, and characters are `Copy`.

### 8.4 Shared Borrows

Shared borrows allow read-only access.

```mo
fn len(s: &String) -> Int
```

Read-only call-site borrowing may be implicit:

```mo
let n = len(name)
```

The compiler treats this as a temporary shared borrow.

### 8.5 Mutable Borrows

Mutable borrows allow exclusive mutation.

```mo
fn clear(s: &mut String)
```

Mutable borrowing at call sites is explicit:

```mo
clear(mut name)
```

### 8.6 Borrow Rules

For a value at any point:

- Any number of shared borrows may exist.
- Or one mutable borrow may exist.
- Shared and mutable borrows cannot overlap.
- A borrowed value cannot be moved until the borrow ends.
- A reference cannot outlive the referenced value.

### 8.7 Lifetimes

Lifetimes are inferred in ordinary code.

The compiler rejects functions whose returned references cannot be proven to come from live inputs or owned storage.

```mo
import * as String from "std/string"

fn bad() -> &String {
    let s = String.new("no")
    &s // error
}
```

Explicit lifetime syntax is postponed. If added, it should be limited to advanced library code.

### 8.8 Drop

Types may conform to `Drop`.

```mo
struct File: Drop {
    fd: Int

    fn drop(self) {
        self.close()
    }
}
```

Drop runs automatically when the owner goes out of scope.

## 9. Interfaces

### 9.1 Interface Declarations

```mo
interface Reader {
    fn read(&mut self, buf: Slice<mut Byte>) -> Result<Int, Error>
}
```

### 9.2 Interface Conformance

```mo
struct File: Reader {
    fd: Int

    fn read(&mut self, buf: Slice<mut Byte>) -> Result<Int, Error> {
        ...
    }
}
```

Conformance is explicit.

### 9.3 Interface Bounds

```mo
fn print<T: Display>(value: T) {
    ...
}
```

### 9.4 Interface Composition

```mo
interface ReadWriter: Reader + Writer {}
```

### 9.5 Dynamic Dispatch

```mo
fn copy_to(writer: &mut dyn Writer) -> Result<(), Error> {
    ...
}
```

### 9.6 Marker Interfaces

Marker interfaces have no methods.

```mo
interface Send {}
interface Sync {}
```

`Send` and `Sync` are compiler-recognized and usually automatically derived.

## 10. Generics

### 10.1 Generic Functions

```mo
fn identity<T>(value: T) -> T {
    value
}
```

Explicit type arguments are substituted into parameter and return types at call sites:

```mo
let answer = identity<Int>(42)
```

Omitted generic function type arguments are inferred from expected return context and argument types when enough information is available:

```mo
let answer: Int = identity(42)
```

### 10.2 Generic Types

```mo
struct Box<T> {
    ...
}
```

### 10.3 Bounds

```mo
fn sort<T: Ord>(items: Slice<mut T>) {
    ...
}
```

Multiple bounds:

```mo
fn hash_key<T: Eq + Hash>(key: &T) -> UInt64 {
    ...
}
```

### 10.4 Implementation Strategy

The initial compiler should monomorphize generic code.

## 11. Error Handling

### 11.1 Result

```mo
enum Result<T, E> {
    Ok(T)
    Err(E)
}
```

### 11.2 Option

```mo
enum Option<T> {
    Some(T)
    None
}
```

### 11.3 Propagation

```mo
fn read_config(path: String) -> Result<Config, Error> {
    let text = fs.read_text(path)?
    parse_config(text)
}
```

### 11.4 Panics

Panics are for unrecoverable programmer errors. They are not normal control flow.

## 12. Unsafe

### 12.1 Unsafe Blocks

```mo
import * as core from "core/unsafe"

unsafe {
    core.write(fd, value)
}
```

### 12.2 Unsafe Operations

Unsafe is required for:

- Raw pointer dereference.
- Calling unsafe functions.
- FFI calls.
- Unchecked casts.
- Manual allocation primitives.
- Implementing unsafe interfaces.

### 12.3 Unsafe Functions

```mo
unsafe fn from_raw(ptr: *mut Byte, len: Int) -> String {
    ...
}
```

Calling an unsafe function requires an unsafe block.

## 13. Foreign Function Interface

### 13.1 Extern Blocks

```mo
extern "C" {
    fn puts(s: *const Byte) -> Int32
}
```

### 13.2 Representation Directives

```mo
@repr(.c)
struct Point {
    x: Float64
    y: Float64
}
```

### 13.3 Calling FFI

FFI calls are unsafe unless explicitly declared safe by a trusted wrapper.

### 13.4 Native Link Dependencies

Packages can declare native linker inputs in `mo.toml`. Native paths are
resolved relative to the manifest that declares them and are collected
transitively when an app imports the package.

```toml
[package]
name = "sqlite"
root = "src"

[native.macos.aarch64]
static_libraries = ["vendor/sqlite/macos-aarch64/libsqlite3.a"]
objects = []
library_paths = []
libraries = []
link_args = [
    "-framework", "Cocoa",
    "-framework", "IOKit",
]
```

Supported native keys:

- `static_libraries`: explicit static archive files such as `libsqlite3.a`.
- `objects`: individual object files such as `sqlite3.o`.
- `library_paths`: directories passed to the linker with `-L`.
- `libraries`: library names passed to the linker with `-l`.
- `link_args`: raw linker arguments for target-specific escape hatches.

Native string arrays may be written on one line or across multiple lines.
`link_args` preserves order and duplicates because many linker options are
positional, such as repeated `-framework` arguments on macOS.

Static libraries are the preferred format for vendored C libraries. A `.a`
file is an archive of `.o` object files; the linker can pull in the needed
members when building the final executable.

Native sections are target-filtered using the same symbols as target
directives. For example, `[native.macos]` applies to all macOS builds and
`[native.macos.aarch64]` applies only to Apple Silicon macOS builds. The
current compiler target is macOS/aarch64.

Mo does not parse C headers. Packages still write the `extern "C"` declarations
that describe the C ABI surface they use.

### 13.5 Manifest Scripts

`mo.toml` can declare project scripts. Scripts are executed from the directory
that contains the manifest, so relative paths are stable for demos and packages.

```toml
[scripts]
prepare = "./prepare-raylib.sh"
build = "mo build app/main.mo -o /tmp/mo_raylib_3d_demo"
run = "mo run app/main.mo"
```

Run a script with:

```sh
mo exec build
mo exec run demos/raylib_3d
```

Extra arguments can be appended after `--`.

## 14. Platform Targets And Conditional Compilation

### 14.1 Initial Target

The first Mo implementation targets macOS only.

Required initial target triples:

```text
aarch64-apple-darwin
x86_64-apple-darwin
```

The compiler may support only one of these first, but the target model must be expressed as a target triple from the beginning.

### 14.2 Target Configuration

The compiler exposes target symbols during conditional compilation.

Required initial target symbols:

```text
.macos
.linux
.windows
.aarch64
.x86_64
.apple
.darwin
.ptr32
.ptr64
```

For `aarch64-apple-darwin`, the active symbols are:

```text
.macos
.aarch64
.apple
.darwin
.ptr64
```

### 14.3 @target Directive

`@target(...)` includes a block only when the condition matches the active target.

```mo
@target(.macos) {
    fn os_name() -> String {
        "macos"
    }
}
```

Supported initial predicates:

```mo
@target(.macos) { ... }
@target(.aarch64) { ... }
@target(all(.macos, .aarch64)) { ... }
@target(any(.aarch64, .x86_64)) { ... }
@target(not(.windows)) { ... }
```

Disabled `@target` blocks are ignored for the active compilation target.

### 14.4 Type Checking Across Targets

The compiler type-checks only enabled items for the active target.

A package should be checkable under each supported target independently. CI or package tooling should run target-specific checks when cross-platform support matters.

### 14.5 Target-Specific Modules

Platform-specific implementation modules are allowed.

```mo
module std.net.darwin

@target(.macos) {
    pub fn make_socket() -> Result<Socket, NetError> {
        ...
    }
}
```

Public standard library APIs should hide platform-specific modules behind stable cross-platform interfaces.

### 14.6 Missing Platform Implementations

If name resolution requires a platform-specific item and no enabled implementation exists for the active target, compilation fails.

The error should identify:

- The missing symbol.
- The active target triple.
- The disabled candidate implementations, if any.

### 14.7 Platform FFI

Platform bindings are ordinary unsafe FFI declarations guarded by `@target`.

```mo
@target(.macos) {
    extern "C" {
        fn getpid() -> Int32
    }
}
```

## 15. Threads

### 15.1 Thread Spawning

Threads are provided by `std.thread`.

```mo
let handle = thread.spawn(move fn() {
    print("running")
})

handle.join()
```

### 15.2 Send

A value must implement `Send` to move into another thread.
Raw pointers and borrowed stack references are not `Send` in spawned thread captures.

### 15.3 Sync

A type implements `Sync` when shared references to it may be accessed from multiple threads safely.

### 15.4 Shared Mutable State

Shared mutable state must use synchronization:

```mo
import * as sync from "std/sync"

let lock = sync.mutex()
sync.lock(lock)
// mutate protected state
sync.unlock(lock)
```

The current executable shared-owner slice is `std/shared.Shared<Int>`:

```mo
import * as shared from "std/shared"

let one = shared.new_int(10)
let two = shared.clone_int(one)
shared.set_int(two, 15)
shared.get_int(one) // 15
```

`Shared<Int>` clones increment a mutex-protected reference count. Dropping each
handle decrements it, and the final drop frees the shared inner allocation.

`std/sync` currently exposes executable pthread-backed `Mutex` and `RwLock`
handles on macOS:

```mo
let rw = sync.rwlock()
sync.read_lock(rw)
sync.rw_unlock(rw)
sync.write_lock(rw)
sync.rw_unlock(rw)
```

The intended long-term generic surface is `Mutex<T>` and `RwLock<T>` with
guard values. The current implementation is a lower-level handle API because
generic ownership-aware guards are not implemented yet.

### 15.5 Channels

Channels support message passing:

```mo
import * as channel from "std/channel"

let ch: channel.Channel<Int> = channel.new()
let worker_ch = channel.clone(ch)

thread.spawn(move fn() {
    channel.send(worker_ch, 1)
})

let value: Int = channel.recv(ch)
channel.close(ch)
channel.destroy(ch)
```

The executable subset currently provides blocking `Channel<Bool>`,
`Channel<Int>`, `Channel<String>`, and `Channel<fn() -> ()>`, backed by pthread
mutexes and condition variables.
Values sent through channels must be `Send`; borrowed references and raw
pointers are rejected by the thread checker at channel-send boundaries.
`new`, `clone`, `send`, `recv`, `close`, and `destroy` infer the concrete
channel implementation from the annotated channel local or sent payload where
possible. Explicit generic calls such as `channel.new<String>()` remain
accepted for simple type arguments; function channels are usually written with
annotated locals such as `let ch: channel.Channel<fn() -> ()> = channel.new()`.
Std channel tests cover int send/recv and string clone/send/recv behavior.
Full payload coverage still requires broader monomorphized storage
support.

### 15.6 Atomics And Tasks

```mo
import * as atomic from "std/atomic"
import * as task from "std/task"

let counter = atomic.int(0)
let handle = task.spawn(move fn() {
    atomic.add(counter, 1)
})
task.join(handle)
```

`std/atomic` currently provides a mutex-backed `AtomicInt`. This is executable
and safe, but not lock-free yet. `std/task` exposes `spawn`, `join`, a fixed
`ThreadPool4` helper, and an initial `TaskQueue4` for submitting `fn() -> ()`
jobs to four workers. Queue destruction is currently a no-op placeholder until
resource destructors are ownership-aware.

## 16. Async And Await

### 16.1 Async Functions

```mo
async fn fetch() -> Result<String, Error> {
    ...
}
```

An `async fn` returns a future.

Conceptually:

```mo
async fn fetch() -> T
```

is a function returning:

```mo
Future<T>
```

### 16.2 Await

```mo
let value = fetch().await
```

Await may suspend the current async computation.

### 16.3 Borrowing Across Await

The compiler checks that references held across `.await` remain valid and do not violate mutable aliasing rules.

### 16.4 Runtime

The language defines async lowering and the `Future` contract. The standard library provides a default executor.

```mo
async.block_on(main())
```

### 16.5 Tasks

```mo
let task = async.spawn(fetch())
let value = task.await?
```

Spawned async tasks must own captured values or capture values that are safe to share.

## 17. Standard Library Reference

### 17.1 Core Prelude

The prelude imports common types and interfaces:

```text
Bool
Int
String
Option
Result
Vec
Box
Copy
Clone
Drop
Display
Debug
Eq
Ord
Hash
Iterator
```

### 17.2 std.core

Contains primitive-adjacent definitions:

- `Option<T>`
- `Result<T, E>`
- `Unit`
- `Never`
- core interfaces

### 17.3 std.mem

Memory and ownership types:

- `Box<T>`
- `Rc<T>`
- `Weak<T>`
- `Arc<T>`
- slice helpers
- raw memory utilities

### 17.4 std.int

Executable integer helpers:

```mo
import * as int from "std/int"

let value = int.parse_decimal("42")
let fallback = int.parse_decimal_or("bad", 7)
let text = int.to_string(value)
let sum = int.checked_add_or(value, 1, 0)
let product = int.checked_mul_or(value, 2, 0)
let byte = int.to_u8_or(value, 0)
let low = int.min(value, 100)
let high = int.max(value, 100)
```

Current scope:

- Decimal `Int` parsing with overflow rejection through `parse_decimal_or`.
- Decimal `Int` formatting through `std/string`.
- `checked_add`, `checked_sub`, and `checked_mul` with zero fallback.
- `checked_add_or`, `checked_sub_or`, and `checked_mul_or` with explicit fallback.
- Width guard/conversion helpers: `is_i8`, `is_u8`, `is_i16`, `is_u16`, `is_i32`, `is_u32`, and matching `to_*_or` helpers.
- `min`, `max`, `min_value`, and `max_value`.

Narrow integer annotations are type checked. Integer literals must fit the
declared width, and plain `Int` variables do not implicitly narrow to small
widths such as `Int8`, `UInt8`, `Int16`, or `UInt16`. Direct Mo calls and
declared extern calls lower annotated integer parameters and returns to their
declared ABI widths. Heap-backed struct fields and enum payloads are stored with
their declared widths and sign- or zero-extended when loaded. Local integer
arithmetic still uses `Int`-sized backend registers, and indirect function
pointer calls still use the current pointer-sized function pointer ABI.

### 17.4.1 std.option

`std/option` provides the standard generic optional-value enum:

```mo
import { Option } from "std/option"

let value: Option<Int> = Some(42)
```

Constructors are `Some(T)` and `None`.

Executable helper APIs:

```mo
option.is_some(Some(1))
option.is_none(None)
option.unwrap_or(Some(42), 0)
option.unwrap_or(None, 7)
option.map(Some(41), add_one)
option.and_then(Some(41), keep_positive)
option.or_else(None, fallback_option)
```

### 17.4.2 std.result

`std/result` provides the standard generic success/error enum:

```mo
import { Result } from "std/result"

let value: Result<Int, Str> = Ok(42)
```

Constructors are `Ok(T)` and `Err(E)`. The `?` operator unwraps `Ok` values
inside functions returning compatible `Result<_, E>`.

Executable helper APIs:

```mo
result.is_ok(Ok(1))
result.is_err(Err(1))
result.unwrap_or(Ok(42), 0)
result.unwrap_or(Err(7), 9)
result.map(Ok(41), add_one)
result.and_then(Ok(41), require_positive)
result.map_err(Err(7), add_one)
result.or_else(Err(7), recover_positive)
```

### 17.5 std.bytes

Executable byte helpers:

```mo
import * as bytes from "std/bytes"

let digit = bytes.digit_value(55)
let is_alpha = bytes.is_alpha(65)
let port = bytes.load_u16_be(ptr, 2)
bytes.store_u32_le(ptr, 0, 16)
```

Current scope:

- ASCII digit, alpha, and whitespace classification.
- Raw pointer byte load/store wrappers.
- String byte reads through `bytes.string_load8`.
- Pointer byte `zero` and `copy` helpers.
- `u16` big/little-endian helpers.
- `u32` little-endian helpers.

Slice and buffer-backed byte APIs belong with the future `alloc.Buffer`,
`Vec<Byte>`, and slice implementation.

### 17.6 std.collections

- `Vec<T>`
- `Map<K, V>`
- `Set<T>`
- `Deque<T>`

Current executable collection coverage includes `Vec<Int>`, `Vec<String>`,
legacy `Vec<fn(Int, &Str) -> Int>` callbacks, typed
`Vec<fn(Int, &http.Request, &Str) -> Int>` middleware callbacks, and
`Map<String, String>` helpers, including standalone generic Vec std tests for
int, string, and handler callbacks, generic string/string map std tests, and
explicit collection destroy coverage.
`std/slice.ByteSlice` has executable tests for string-backed byte views,
subslice clamping, bounded `expr[index]` indexing, and borrowed-backing memory
behavior.
`std/buffer` has executable tests for append, finish, destroy, and growth past
initial capacity. It exposes first-class `buffer.StringBuilder` and
`buffer.ByteBuffer` owner types. `string_builder_new` returns
`buffer.StringBuilder`, with `string_builder_append`,
`string_builder_append_byte`, `string_builder_append_int`,
`string_builder_finish`, `string_builder_capacity`,
`string_builder_remaining`, and `string_builder_destroy` operating on that
type. `byte_buffer_new` returns `buffer.ByteBuffer`, with `byte_buffer_push`,
`byte_buffer_get`, `byte_buffer_set`, `byte_buffer_finish`,
`byte_buffer_length`, `byte_buffer_capacity`, `byte_buffer_remaining`, and
`byte_buffer_destroy` operating on that type. Bounded byte get/set return `-1`
for out-of-range indexes. `finish`, `string_builder_finish`, and
`byte_buffer_finish` transfer the backing string to the returned `String`, so
they are treated as consuming cleanups by drop planning. The promoted owner
types have std, compile/run memory, auto-drop, IR, and drop-check coverage.
Fully generic collection algorithms remain part of the active roadmap.

The CLI suite also runs the remaining passing `std/test/*.test.mo` files for
async, atomics, bytes, filesystem, integers, IO, networking, paths, process,
shared ownership, SSE, strings, sync, tasks, and threads.

### 17.7 std.io

Core interfaces:

```mo
interface Reader {
    fn read(&mut self, buf: Slice<mut Byte>) -> Result<Int, IOError>
}

interface Writer {
    fn write(&mut self, buf: Slice<Byte>) -> Result<Int, IOError>
}
```

### 17.8 std.fs

Filesystem APIs:

```mo
fn open_read(path: &Str) -> Int
fn open_write_truncate(path: &Str) -> Int
fn close_fd(fd: Int) -> Int
fn remove(path: &Str) -> Int
fn read_text(path: &Str) -> String
fn read_text_or(path: &Str, fallback: &Str) -> String
fn write_text(path: &Str, text: &Str) -> Int
fn exists(path: &Str) -> Bool
```

The current macOS implementation avoids variadic `open(path, flags, mode)` for file creation because Darwin arm64 uses a distinct varargs ABI. `std/fs` uses fixed-ABI creation/truncate calls internally and keeps that platform detail out of user code.

### 17.8.1 std.process

Process APIs:

```mo
fn current_dir() -> String
fn executable_path() -> String
```

### 17.8.2 std.path

Path APIs:

```mo
fn separator() -> String
fn join(base: &Str, child: &Str) -> String
```

### 17.9 std.thread

- `spawn`
- `join`
- `JoinHandle`

Current executable scope: `thread.spawn` accepts named `fn() -> ()` tasks,
non-capturing closure tasks, and moved captures for pointer-sized scalar and
heap values, returning a `JoinHandle` that can be passed to `thread.join`. The
captured environment is freed after the thread task returns. Captured
heap-owned structs/enums are recursively dropped by the generated thread
closure on normal task completion and before explicit unit `return` exits.
Fully path-sensitive drop cleanup for every return expression shape is still
part of the broader drop-path completion work.

### 17.10 std.sync

- `Mutex<T>`
- `RwLock<T>`
- `Once<T>`
- `Atomic<T>`
- `Channel<T>`

### 17.11 std.async

- `Future<T>`
- `Task<T>`
- `spawn`
- `block_on`
- timers
- async channels
- threadpool-backed executor

Current executable scope: `std.async.spawn` and `std.async.join` wrap the
pthread-backed thread task machinery for `fn() -> ()` tasks. This is an
executor brick, not full future polling yet.

`std.async.block_on(Int)` is also executable as the first immediate executor
boundary. It returns the already-evaluated `Int` value produced by the current
immediate async lowering:

```mo
import * as async from "std/async"

async fn load() -> Int {
    return 42
}

fn main() -> Int {
    return async.block_on(load())
}
```

Value-producing `.await` is executable for this immediate model:

```mo
async fn add_two() -> Int {
    let value = load().await
    return value + 2
}
```

This is not yet a heap-stored, poll-driven future executor.

### 17.12 std.net

Networking primitives:

- `SocketAddr`
- `TcpListener`
- `TcpStream`
- `UdpSocket`

Initial synchronous TCP helpers:

```mo
fn tcp_listen_ephemeral(backlog: Int) -> Int
fn listener_new(backlog: Int) -> Int
fn listener_accept(listener: Int) -> Int
fn listener_close(listener: Int) -> Int
fn stream_connect_loopback(port: Int) -> Int
fn stream_read_byte(stream: Int) -> Int
fn stream_write(stream: Int, text: &Str) -> Int
fn stream_close(stream: Int) -> Int
```

Typed wrapper helpers are also executable:

```mo
fn tcp_listener_new(backlog: Int) -> TcpListener
fn tcp_listener_port(listener: &TcpListener) -> Int
fn tcp_listener_accept(listener: &TcpListener) -> TcpStream
fn tcp_listener_close(listener: &TcpListener) -> Int

fn tcp_stream_from_fd(fd: Int) -> TcpStream
fn tcp_stream_connect_loopback(port: Int) -> TcpStream
fn tcp_stream_fd(stream: &TcpStream) -> Int
fn tcp_stream_read_byte(stream: &TcpStream) -> Int
fn tcp_stream_write(stream: &TcpStream, text: &Str) -> Int
fn tcp_stream_close(stream: &TcpStream) -> Int
```

`TcpListener` and `TcpStream` are unique owned wrappers. Locals of these typed
wrapper types close automatically when they leave scope, including early
returns. Calling `tcp_listener_close` or `tcp_stream_close` explicitly releases
the handle early and suppresses the automatic close for that local. Raw `Int`
socket/fd helpers remain manual lifecycle APIs.

### 17.12.1 std.event

The initial event-loop abstraction wraps the current `select` readiness path:

```mo
import * as event from "std/event"
import * as net from "std/net"

let loop = event.new()
let listener = net.tcp_listener_new(16)
event.wait_listener(loop, listener)
let stream = net.tcp_listener_accept(listener)
event.wait_stream(loop, stream)
```

Current executable helpers:

```mo
fn new() -> EventLoop
fn backend(loop: &EventLoop) -> Int
fn wait_readable_fd(loop: &EventLoop, fd: Int) -> Int
fn wait_writable_fd(loop: &EventLoop, fd: Int) -> Int
fn wait_listener(loop: &EventLoop, listener: &TcpListener) -> Int
fn wait_stream(loop: &EventLoop, stream: &TcpStream) -> Int
fn wait_stream_writable(loop: &EventLoop, stream: &TcpStream) -> Int
```

### 17.12.2 std.async_tcp

The current executable async TCP slice is a readiness-gated helper layer over
`std.event` and typed `std.net` handles. It waits on the event loop before
performing the underlying synchronous TCP operation. Full `async fn` future
polling remains part of the executor milestone.

```mo
import * as async_tcp from "std/async_tcp"
import * as event from "std/event"
import * as net from "std/net"

let loop = event.new()
let listener = net.tcp_listener_new(16)
let client = net.tcp_stream_connect_loopback(net.tcp_listener_port(listener))
let server = async_tcp.accept(loop, listener)
async_tcp.write(loop, client, "A")
let byte = async_tcp.read_byte(loop, server)
```

Current executable helpers:

```mo
fn accept(loop: &EventLoop, listener: &TcpListener) -> TcpStream
fn read_byte(loop: &EventLoop, stream: &TcpStream) -> Int
fn write(loop: &EventLoop, stream: &TcpStream, text: &Str) -> Int
```

Target async APIs:

```mo
async fn TcpListener.accept(&self) -> Result<TcpStream, NetError>
async fn TcpStream.read(&mut self, buf: Slice<mut Byte>) -> Result<Int, NetError>
async fn TcpStream.write(&mut self, buf: Slice<Byte>) -> Result<Int, NetError>
```

### 17.13 std.http

HTTP server primitives:

- `Server`
- `Router`
- `Request`
- `Response`
- `Method`
- `Status`
- `HeaderMap`
- `Body`

Representative API:

```mo
struct Server {
    fn new() -> Server
    fn workers(self, count: Int) -> Server
    fn get(self, path: String, handler: Handler) -> Server
    fn post(self, path: String, handler: Handler) -> Server
    async fn listen(self, addr: String) -> Result<(), HttpError>
}

type Handler = async fn(Request) -> Result<Response, Error>
```

Current executable `std/http.Request` coverage is intentionally narrow but
typed. `http.read_request(fd)` parses the benchmark request line plus headers
into:

- `method`: `1` for GET and `2` for POST in the current slice.
- `method_name`: an owned string containing the parsed request method token.
- `path`: an owned string containing the parsed request path token.
- `route_id`: `1` for `GET /pokemon`, `2` for `POST /pokemon`, and `3` for `GET /health`.
- `content_length`: the parsed numeric request body length from headers.
- `body`: an owned string containing the request body bytes read from the fd
  according to `content_length`.
- request-owned header storage for arbitrary syntactically valid header lines.

`http.request_route(fd)` remains available for existing router code as a
lightweight route-only parser. Use `read_request(fd)` when method, body length,
or body data is needed. `http.read_body(fd, content_length)` remains available
as a lower-level helper only when the fd has not already been consumed by
`read_request`. Use `http.request_header_count(request)` to inspect the parsed
header count, `http.request_header(request, name)` to read a fresh owned copy of
a parsed header value, and `http.request_destroy(request)` to release
request-owned method/path/body strings and header storage.

`http.HeaderMap` is available as a collection-backed header store for current
HTTP code:

```mo
let headers = http.headers_new()
http.headers_put(headers, String.from("Content-Type"), String.from("application/json"))
let content_type = http.headers_get(headers, "Content-Type")
http.headers_destroy(headers)
```

Current `HeaderMap` storage uses Vec-backed string/string map storage under the
HTTP API. `headers_get` returns a fresh owned string copy; duplicate names use
the latest stored value because lookup scans from the end. `headers_destroy`
releases the stored key/value strings and backing slots exactly once. Parsed
requests use the same Vec-backed string/string storage shape internally for
arbitrary exact-name headers; header lookup is not case-insensitive yet.

Current executable `std/http.Response` coverage is typed and owns its render
payload:

- `status`: numeric HTTP status for the current renderer.
- `body`: owned response body string.
- `content_type`: owned response content type string.
- response headers: owned Vec-backed string/string header storage.

`http.json_response`, `http.created_json_response`,
`http.bad_request_response`, `http.not_found_response`, and
`http.internal_server_error_response` construct `Response` values.
`http.render` turns an owned `Response` into the wire string, and
`http.write_response` writes a rendered response to a file descriptor.
`http.render_response` remains as the explicit body/content-type renderer used
by older tests and compatibility helpers. Existing `write_json`,
`write_created_json`, and `write_not_found` helpers keep their direct
compatibility rendering path. Express route handlers use the
`fn(Int, &http.Request, &Str) -> http.Response` shape. `http.status_ok`, `http.status_created`, and
`http.status_bad_request`, `http.status_not_found`, and
`http.status_internal_server_error` provide the current numeric status
constants.

`http.response_header_put`, `http.response_header`,
and `http.response_header_count` expose current response header storage.
`response_header_put` takes owned name/value strings; duplicate names are stored
and lookup returns the latest matching value. Rendered responses always include
the built-in `Content-Type`, `Content-Length`, and `Connection: close` headers,
with custom response headers written between content length and connection.

Response helpers:

```mo
struct Response {
    fn text(body: String) -> Response
    fn json<T: JsonEncode>(value: T) -> Result<Response, JsonError>
    fn sse(stream: SseStream) -> Response
}
```

Current executable demo:

```sh
mo build examples/demo/pokemon_server.mo -o /tmp/mo-pokemon-server
/tmp/mo-pokemon-server
```

The demo prints an ephemeral loopback port and serves a finite number of
synchronous requests:

- `GET /pokemon` returns the current Pokémon JSON resource.
- `POST /pokemon` trains the Pokémon and persists the incremented level.
- `GET /health` returns `{"status":"ok"}`.
- other paths return `404 Not Found`.

### 17.14 Userland JSON

JSON support is a userland package, not part of `std`.

- `JsonValue`
- `JsonError`
- `JsonEncode`
- `JsonDecode`
- parser
- encoder

Current executable userland package:

```mo
import * as pokemon from "../../lib/pokemon"

let pikachu = pokemon.starter()
let text = pokemon.encode(pikachu)
let parsed = pokemon.parse_or(text, pikachu)
```

`lib/pokemon.mo` composes `lib/json.mo` with `std/fs` for a stateful Pokémon JSON file used by the server benchmark.

`lib/express.mo` is reusable userland HTTP/TCP plumbing over `std/net` and `std/http`. It does not import application resources. Its current registration API mutates an `App` through method-call syntax:

```mo
let mut server = express.with_backlog(128)
server.use_before(before_request)
server.get("/pokemon", get_pokemon)
server.post("/pokemon", post_pokemon)
server.get("/health", get_health)
let routes = express.route_count(server)
```

`App` stores route method IDs, owned route paths, handler pointers, and
before-request middleware callbacks in Vec-backed tables. `handle_once` and the
queued threadpool request path call `http.read_request`, match the registered
method/path pair, and pass `&http.Request` to middleware and route handlers.
Route handlers return typed `http.Response` values, which Express writes to the
client fd.
`use_before` appends `fn(Int, &http.Request, &Str) -> Int` middleware callbacks.
Middleware callbacks run in registration order.

```mo
fn before_request(client: Int, request: &http.Request, context: &Str) -> Int {
    return 0
}
```

`lib/pokemon_server.mo` is the Pokémon-specific userland server package. It composes `lib/express.mo` and `lib/pokemon.mo` to serve `GET /pokemon`, `POST /pokemon`, `GET /health`, install a before-request hook, and run the current fixed four-worker server smoke path.

Representative API:

```mo
fn parse(text: String) -> Result<JsonValue, JsonError>
fn stringify(value: JsonValue) -> Result<String, JsonError>

interface JsonEncode {
    fn encode_json(&self) -> Result<JsonValue, JsonError>
}

interface JsonDecode {
    fn decode_json(value: JsonValue) -> Result<Self, JsonError>
}
```

Compiler-derived JSON implementations may be added later.

### 17.15 std.sse

Server-sent events support:

- `SseStream`
- `SseSender`
- event name
- event id
- retry interval
- streaming HTTP body integration

Representative API:

```mo
fn stream(make: async fn(SseSender) -> Result<(), Error>) -> SseStream

struct SseSender {
    async fn send(&mut self, event: String, data: JsonValue) -> Result<(), Error>
}
```

### 17.16 std.test

Test declarations:

```mo
test "name" {
    assert(true)
}
```

The command-line test runner is:

```text
mo test <file-or-dir>
```

`mo test` compiles each test body as a temporary zero-argument `main`, runs it,
and treats process exit code `0` as pass. Nonzero exits fail the test.
`assert(Bool)` is compiler-known in executable tests, prints `assertion failed`,
and returns exit code `1` when the condition is false.

Lexer and parser failures reported by CLI package-loading commands include the
source location:

```text
path/to/file.mo:line:column: parse error [MO0002]: expected ...
```

Some semantic diagnostics also include locations as their checks gain span
coverage:

```text
path/to/file.mo:line:column: semantic error [MO1001]: duplicate top-level symbol `name`
```

## 18. Compiler IR

### 18.1 Pipeline

```text
Source
  -> Lexer
  -> Parser
  -> AST
  -> Name resolution
  -> Type checking
  -> Ownership and borrow checking
  -> Mo IR
  -> Optimization
  -> Backend lowering
```

### 18.2 Backend Targets

Supported or planned backends:

- LLVM.
- Cranelift.
- C.
- WASM.
- Custom native backend.

### 18.3 Mo IR Requirements

Mo IR must be:

- Typed.
- Control-flow explicit.
- SSA-based or SSA-friendly.
- Backend-independent.
- Ownership-aware.
- Drop-aware.
- Suitable for async state-machine lowering.

### 18.4 Example IR

```text
fn add(Int a, Int b) -> Int {
block0:
    %0 = add_int a, b
    return %0
}
```

Ownership-aware example:

```text
fn main() -> Unit {
block0:
    %s = call String.new("hello")
    call print(&%s)
    drop %s
    return ()
}
```

### 18.5 Async Lowering

An async function lowers to a state machine with:

- Captured locals.
- Resume points.
- Poll function.
- Completion state.
- Drop logic for partially completed futures.

### 18.6 Drop Lowering

The compiler inserts explicit `drop` operations in Mo IR after ownership and borrow checking.

Drops must run:

- At normal scope exit.
- During early return.
- During error propagation.
- During panic unwinding if unwinding is supported.

## 19. Implementation Milestones

### 19.1 M0: Parser And Tiny Core

- Lexer.
- Parser.
- AST.
- Variables.
- Primitive types.
- Functions.
- Structs.
- Methods.
- Basic Mo IR.
- One backend.
- macOS target triple support.
- `@target` filtering.
- `test` item parsing.
- `mo test` native execution for backend-supported test bodies.

### 19.2 M1: Safety Core

- Enums.
- Pattern matching.
- Ownership.
- Moves.
- Borrows.
- Drop insertion.
- `Option`.
- `Result`.

### 19.3 M2: Usable Language

- Generics.
- Interfaces.
- Modules.
- Unsafe.
- C FFI.
- Collections.
- Filesystem and IO.
- Test body execution.
- JSON parser and encoder.

### 19.4 M3: Concurrency

- Threads.
- `Send`.
- `Sync`.
- Mutexes.
- Atomics.
- Channels.
- `Arc<T>`.
- Threadpool executor foundation.
- macOS thread and socket backends.

### 19.5 M4: Async

- `async fn`.
- `.await`.
- Future interface.
- Async lowering.
- Default executor.
- Async IO foundations.
- Async TCP.
- HTTP server.
- Server-sent events.

## 20. Deferred Features

These features are intentionally postponed:

- Explicit lifetime parameters.
- Macros.
- Compile-time reflection.
- Specialization.
- Higher-ranked interface bounds.
- Interface associated types.
- Async methods in interfaces.
- Custom allocator parameters everywhere.
- Coroutine syntax beyond async.
- Effect systems.
- Dependent types.

## 21. Unresolved Reference Questions

- Exact grammar and operator precedence.
- Exact semicolon rules.
- Whether read-only borrows are always implicit at call sites.
- Whether mutable borrows use `mut value`, `&mut value`, or both.
- Whether `go { ... }` should be syntax or a stdlib function.
- Whether `main` may be `async fn main`.
- Whether integer overflow traps in debug and wraps in release.
- Whether panics unwind or abort by default.
- How dynamic interface objects are represented.
- How much ABI stability the language guarantees.
- Exact target-specific file/module selection rules.
- Whether `@target` can apply to expressions as well as item blocks.
