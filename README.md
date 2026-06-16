# Mo

Mo is an experimental systems programming language and compiler.

Current status: pre-alpha. The compiler, runtime, standard library, and package model are still changing quickly.

## Install

Install the latest pre-alpha release:

```sh
curl -fsSL https://raw.githubusercontent.com/olup/mo/main/scripts/install.sh | sh
```

The installer adds `mo` to your shell profile. Restart your shell, or source the
profile printed by the installer.

To skip shell profile changes:

```sh
curl -fsSL https://raw.githubusercontent.com/olup/mo/main/scripts/install.sh | MO_INSTALL_UPDATE_PROFILE=0 sh
```

To install somewhere else:

```sh
curl -fsSL https://raw.githubusercontent.com/olup/mo/main/scripts/install.sh | MO_PREFIX=/opt/mo sh
```

The installer selects one of these release archives:

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
