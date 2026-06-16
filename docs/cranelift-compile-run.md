# Cranelift Compile-Run Milestone

Goal: compile and run a minimal Mo program on macOS using Cranelift, without making Cranelift assumptions leak into AST/HIR.

## First Target Program

```mo
fn main() -> Int {
    return 0
}
```

The first working command should be:

```text
mo build examples/compile/return_zero.mo -o build/return_zero
```

Then:

```text
build/return_zero
```

should exit with status `0`.

Status: implemented and verified.

Verification command:

```text
cargo run -- build examples/compile/return_zero.mo -o build/return_zero
build/return_zero
```

The executable exits with status `0`.

## CLI Tooling

Current useful commands:

```text
mo fmt <file-or-dir>
mo fmt <file-or-dir> --check
mo test <file-or-dir> --filter <text> --json --timeout <seconds>
mo test <file-or-dir> --watch --filter <text>
mo test <file-or-dir> --list
mo build <file> -o <path> --emit-hir --emit-ir --dump-drops --dump-ownership
mo run <file> --watch --debounce-ms 250
mo run <file> --metrics --metrics-json build/metrics.json --memory-report
mo example <name-or-path>
mo smoke pokemon-server
mo bench pokemon-server -- --total 1000 --concurrency 32
```

Notes:

- `mo fmt` is intentionally simple today: it normalizes indentation/blank lines for `.mo` source files.
- `mo test` discovers `.mo` tests, can filter/list them, emits line-delimited JSON when requested, can kill hung native test binaries with `--timeout`, and can poll/re-run with `--watch`.
- `--emit-hir`, `--emit-ir`, `--dump-drops`, and `--dump-ownership` expose compiler internals for ownership/drop debugging.
- `mo run --watch` polls the entry file plus `core`, `std`, and `lib` `.mo` files; on change it kills the previous child, rebuilds, and restarts after successful builds.
- `--memory-report` currently reports process RSS and points users at runtime counter APIs (`core.mem_alloc_count`, `core.mem_free_count`, `core.mem_live_bytes`, `core.mem_high_water_bytes`) for program-level counted-runtime assertions.
- `mo smoke pokemon-server` / `mo bench pokemon-server` wrap `scripts/load_pokemon_server.py`.

## Required Work

- Add `mo build <file> -o <path>`. Done.
- Keep `mo check` as frontend-only validation.
- Reuse the existing frontend pipeline through HIR, safety checks, drop planning, and Mo IR.
- Add a backend abstraction so Cranelift is not hard-coded into frontend modules.
- Add a Cranelift backend module. Done.
- Lower the supported subset to Cranelift IR:
  - zero-argument `main`. Done.
  - `Int` return values. Done.
  - integer literals. Done.
  - direct `return`. Done.
- Emit a native object file. Done.
- Link an executable on macOS. Done.
- Add tests that build and run the first target program. Done.

## Immediate Non-Goals

- heap strings and general string operations.
- higher-order calls and generic calls.
- heap-owned string operations.
- async lowering to executable state machines.
- web server runtime.
- LLVM backend.

## Next Compile Targets

1. `fn main() -> Int { return 0 }`
2. `fn main() -> Int { return 42 }`
3. local integer binding and return. Done.
4. integer arithmetic. Done.
4.1. unary negative integer literals. Done.
5. direct user function calls. Done for integer arguments/return values.
6. `print("hello")` through a runtime symbol. Done through imported `puts` and static string data.
7. `if` branches over boolean literals and integer comparisons. Done.
8. mutable integer locals and `while` loops. Done.
9. local struct construction and integer field access. Done.
10. `print(Int)` for small non-negative integers. Done through imported `putchar`.
11. combined normal-program smoke with helper calls, loops, branches, local struct fields, and printing. Done.
12. pointer-backed struct values passed to and returned from functions. Done.
13. string literals as runtime pointer values passed to and returned from functions. Done.
14. struct string fields and `print(user.name)`. Done.
15. Option-like enum construction and `return match` over integer payloads. Done for `Some(Int)`/`None`.
16. Result/custom enum construction and integer match expressions. Done for `Ok`/`Err`, custom zero-or-one-payload variants, `return match`, and `let x = match ...`.
17. Multi-payload enum variants, recursive enum/struct drops, and pointer-sized string/struct match results, including direct payload binding observation. Done.
18. First executable std string surface. Done for imported `std/string` `String.new`, `String.len`, `String.concat`, `String.from_int`, and `String.from_byte` over private core string bricks.
19. First executable std IO surface. Done for `core.write(fd, string)` through imported `write` with `strlen`-computed byte length.
20. Declared narrow integer direct function/extern ABI plus packed heap struct/enum storage. Done for `Int8`, `UInt8`, `Int16`, `UInt16`, and `Int32` smoke coverage.
