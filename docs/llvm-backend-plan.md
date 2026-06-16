# LLVM Backend Plan

Goal: add an LLVM backend for Mo that can compile the existing Mo IR to native object files without leaking LLVM assumptions into AST, HIR, type checking, or safety checks.

The first target should be feature parity for a small executable subset on the current host target, then incremental parity with the existing Cranelift backend.

## Current Backend Shape

The compiler already has a backend-friendly pipeline:

1. Parse and load a package.
2. Run semantic, resolve, type, ownership, borrow, drop, and thread checks.
3. Lower HIR to `IrProgram`.
4. Emit native code from `IrProgram`.

Today `build_program` hard-codes `Target::macos_aarch64()`, emits through `backend::cranelift`, and links the resulting object with `cc`. The Cranelift backend owns several concerns that should be separated before adding LLVM:

- target selection
- backend selection
- object file emission
- linker invocation
- pointer-sized layout rules
- runtime symbol declarations
- struct and enum layout helpers

## Non-Goals

- Do not rewrite the frontend.
- Do not change Mo source semantics.
- Do not make LLVM the only backend.
- Do not add Wasm support as part of this effort.
- Do not move stdlib behavior into the compiler.
- Do not attempt full optimization work before correctness parity.

## Phase 1: Backend Selection

Add an explicit backend configuration type.

Suggested API shape:

```rust
pub enum BackendKind {
    Cranelift,
    Llvm,
}

pub struct BuildOptions {
    pub backend: BackendKind,
    pub target: Target,
}
```

Update the CLI:

```text
mo build <file> -o <output> --backend cranelift
mo build <file> -o <output> --backend llvm
```

Default to Cranelift initially so existing behavior stays stable.

Acceptance criteria:

- `mo build` with no `--backend` still uses Cranelift.
- `--backend cranelift` behaves exactly like the current build path.
- `--backend llvm` returns a clear "not implemented" diagnostic until phase 3.
- Tests cover CLI parsing and backend dispatch.

## Phase 2: Extract Shared Backend Infrastructure

Move backend-neutral helpers out of `backend/cranelift.rs`.

Candidates:

- backend function signatures
- value size and alignment
- struct layout
- enum layout
- integer signedness helpers
- target pointer width
- runtime symbol grouping

Suggested files:

```text
src/backend/layout.rs
src/backend/symbols.rs
src/backend/link.rs
src/backend/llvm.rs
```

The layout module should take target data instead of assuming 64-bit pointers. Native LLVM can still use 64-bit pointers for the initial macOS target, but the abstraction should not bake in that assumption.

Acceptance criteria:

- Cranelift behavior is unchanged.
- Layout tests assert current struct and enum offsets.
- Backend modules no longer duplicate target-independent layout logic.

## Phase 3: LLVM MVP

Implement enough LLVM lowering for the first executable target:

```mo
fn main() -> Int {
    return 0
}
```

Recommended Rust dependency options:

- `inkwell`, if using installed LLVM bindings is acceptable.
- `llvm-sys`, if direct control matters more than ergonomics.
- textual LLVM IR emission plus `llc`/`clang`, if a simpler bootstrapping path is preferred.

For the MVP, textual LLVM IR is the lowest-risk path because it avoids binding/version churn and makes generated output easy to inspect. A later phase can replace it with bindings if needed.

Initial lowering support:

- module creation
- `main` function
- integer constants
- direct return
- object emission through `clang` or `llc`
- existing native link path

Acceptance criteria:

- `mo build examples/compile/return_zero.mo -o build/return_zero_llvm --backend llvm` succeeds.
- The produced executable exits with status `0`.
- A CLI smoke test verifies build and run.

## Phase 4: Core Control Flow And Calls

Add parity for the early Cranelift milestone examples:

- integer literals and returns
- local integer bindings
- integer arithmetic
- direct user function calls
- boolean literals
- integer comparisons
- `if` branches
- `while` loops

LLVM lowering requirements:

- allocate SSA slots or use LLVM SSA values for locals
- generate basic blocks for Mo IR blocks
- lower `IrTerminator::Return`, `Jump`, and `Branch`
- lower direct calls using Mo backend symbols
- preserve declared integer widths in ABI-facing positions

Acceptance criteria:

- LLVM passes the same compile/run tests as Cranelift for integer-only examples.
- Generated LLVM IR can be dumped with a hidden or test-only option for debugging.

## Phase 5: Static Data And Strings

Add support for string literals and current string runtime operations.

Lowering requirements:

- static string data with null terminators where libc calls require C strings
- string literal values as pointers
- imported calls for `puts`, `strlen`, `write`, `putchar`, `malloc`, `free`, and `memcpy`
- `StringLen`, `StringPtr`, string compare, concat, int-to-string, and from-pointer operations as currently represented in Mo IR

Acceptance criteria:

- `print("hello")` works.
- `print(Int)` works for the currently supported range.
- `std/string` compile/run examples pass under LLVM.
- `core.write` examples pass under LLVM.

## Phase 6: Heap Structs, Enums, And Drops

Implement pointer-backed aggregate behavior.

Lowering requirements:

- heap allocation for structs and enums
- packed field and payload stores using shared layout helpers
- field loads
- enum tag loads and match dispatch
- recursive drop glue
- calls to `free`

Acceptance criteria:

- struct field examples pass.
- Option/Result/custom enum match examples pass.
- recursive enum drop smoke tests pass.
- LLVM and Cranelift agree on layout tests.

## Phase 7: Function Values And Indirect Calls

Add function pointer support.

Lowering requirements:

- named function address values
- function-typed locals and fields
- indirect calls
- function pointers in heap structs, enums, and channels

Acceptance criteria:

- function pointer roundtrip examples pass.
- channel/function task examples compile once thread support exists.

## Phase 8: Threads And Native Runtime Imports

Add parity for native thread primitives.

Lowering requirements:

- `raw_thread_spawn`
- `raw_thread_join`
- thread trampoline functions
- captured environment allocation and cleanup
- imported `pthread_create` and `pthread_join`

Acceptance criteria:

- `std/thread` tests pass under LLVM.
- `std/task` fixed thread pool examples pass under LLVM.
- thread checker behavior remains frontend-owned and unchanged.

## Phase 9: Stdlib And Server Parity

Bring LLVM to parity with the current native demo surface.

Targets:

- `std/fs`
- `std/io`
- `std/net`
- `std/sync`
- `std/channel`
- `std/atomic`
- `std/task`
- Pokémon REST/server examples

Acceptance criteria:

- all existing Cranelift compile/run examples that are target-compatible pass under LLVM.
- benchmark server builds and serves the same smoke requests.

## Testing Strategy

Use the existing compile examples as the parity ladder.

Add backend-parametrized helpers so each executable smoke can run against both backends without duplicating test bodies.

Recommended test groups:

- backend dispatch tests
- layout unit tests
- LLVM textual IR snapshot tests for small programs
- compile-only tests for unsupported runtime features during early phases
- compile-and-run tests for each completed feature group

Each phase should add one or more examples to the LLVM allowlist. Avoid flipping all compile/run tests to LLVM until runtime parity is real.

## Risk Areas

- LLVM version management if using bindings.
- ABI mismatches for narrow integer arguments and returns.
- Pointer-width assumptions currently embedded in backend layout.
- Keeping generated symbol names identical across backends.
- Debugging differences between LLVM optimization levels and Cranelift behavior.
- Runtime calls that accidentally rely on Darwin-specific C ABI details.

## Recommended First Patch

The first implementation patch should only add backend selection and preserve current behavior.

Suggested scope:

1. Add `BuildOptions` and `BackendKind`.
2. Change `build_program(program, output)` to call `build_program_with_options`.
3. Add `--backend cranelift|llvm` parsing.
4. Keep Cranelift as the default.
5. Add a stub `backend::llvm::emit_object` returning a diagnostic.
6. Add CLI tests for default backend, explicit Cranelift, and LLVM-not-implemented.

This creates the extension point without destabilizing the code generator.
