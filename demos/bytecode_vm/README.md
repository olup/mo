# Bytecode VM Demo

This demo is a compact compiled-language workload:

- a bounded bytecode interpreter loop,
- mutable CPU state,
- deterministic checksums,
- four concurrent VM jobs through `std/task`,
- explicit ownership cleanup around the worker job cells.

Run it from the repository root:

```sh
./target/debug/mo build demos/bytecode_vm/main.mo -o /tmp/mo_bytecode_vm
/tmp/mo_bytecode_vm
```

Expected exit code: `42`.
