# Releasing Mo

Mo releases are driven by the crate version in `Cargo.toml` and a matching Git tag.

## Release flow

1. Update `Cargo.toml`:

   ```toml
   version = "0.2.0"
   ```

2. Run the local checks:

   ```sh
   cargo fmt --check
   cargo test --locked
   cargo build --locked --release
   ```

3. Commit the version bump.

4. Create and push a matching tag:

   ```sh
   git tag v0.2.0
   git push origin main --tags
   ```

The release workflow rejects tags that do not match the `Cargo.toml` version.

## Artifacts

The GitHub release publishes compiler packages for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

Each archive contains:

- the `mo` compiler binary
- `std/`, `core/`, `alloc/`, and `lib/`
- `docs/`
- `VERSION`
