# Mo

Mo is an experimental systems programming language and compiler.

Current status: pre-alpha. The compiler, runtime, standard library, and package model are still changing quickly.

## Install

Download the archive for your platform from the latest GitHub release:

```sh
curl -L -o mo.tar.gz \
  https://github.com/olup/mo/releases/download/v0.1.0-pre-alpha/mo-aarch64-apple-darwin.tar.gz
tar -xzf mo.tar.gz
cd mo-aarch64-apple-darwin
./mo check examples/compile/print_hello.mo
```

Pick the archive that matches your machine:

- `mo-aarch64-apple-darwin.tar.gz` for Apple Silicon macOS
- `mo-x86_64-apple-darwin.tar.gz` for Intel macOS
- `mo-aarch64-unknown-linux-gnu.tar.gz` for ARM64 Linux
- `mo-x86_64-unknown-linux-gnu.tar.gz` for x86_64 Linux

The archive includes the `mo` binary plus `std/`, `core/`, `alloc/`, and `lib/`.
For this pre-alpha, keep those directories next to the binary, or set `MO_ROOT`
to the directory that contains them:

```sh
export MO_ROOT=/opt/mo
export PATH="$MO_ROOT:$PATH"
mo check examples/compile/print_hello.mo
```

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
