# Mo

Mo is an experimental systems programming language and compiler.

Current status: pre-alpha. The compiler, runtime, standard library, and package model are still changing quickly.

## Local development

```sh
cargo fmt --check
cargo test --locked
cargo build --locked --release
```

Run a Mo source file:

```sh
cargo run -- run examples/compile/print_hello.mo
```

## Releases

Releases are created from matching `Cargo.toml` versions and Git tags. See `docs/releasing.md`.
