# Mo

Mo is an experimental systems programming language and compiler.

Current status: pre-alpha. The compiler, runtime, standard library, and package model are still changing quickly.

## Install

The repository is private, so install from an authenticated `gh` session:

```sh
case "$(uname -s)-$(uname -m)" in Darwin-arm64) p=mo-aarch64-apple-darwin;; Darwin-x86_64) p=mo-x86_64-apple-darwin;; Linux-aarch64|Linux-arm64) p=mo-aarch64-unknown-linux-gnu;; Linux-x86_64) p=mo-x86_64-unknown-linux-gnu;; *) echo "unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 1;; esac; d="$(mktemp -d)"; gh release download v0.1.0-pre-alpha -R olup/mo -p "$p.tar.gz" -O "$d/mo.tar.gz" && rm -rf "$HOME/.local/mo" && mkdir -p "$HOME/.local/mo" && tar -xzf "$d/mo.tar.gz" -C "$HOME/.local/mo" --strip-components=1 && export PATH="$HOME/.local/mo:$PATH" && printf 'fn main() -> Int {\n    return 0\n}\n' > "$d/smoke.mo" && mo check "$d/smoke.mo"
```

Add this to your shell profile to keep `mo` on your `PATH`:

```sh
export PATH="$HOME/.local/mo:$PATH"
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
