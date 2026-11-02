# Rust Bundling

Source: [infra/shared/rust-bundle.ts](../../../infra/shared/rust-bundle.ts).

## What

Explicit cargo-lambda packaging and static-file copying for SST.

## Why

Both Rust binaries have nested manifests and need reproducible ARM64 bootstrap bundles.

## How

Builds with --release --locked --arm64 and an explicit manifest into .sst/rust/<stage>/<name>. Input mtimes skip fresh builds; target directories are excluded from freshness traversal. Public auth copies auth.clients.toml. Missing output fails deployment preparation.
