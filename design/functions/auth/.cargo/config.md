# Cargo Target Settings

Source: [packages/functions/auth/.cargo/config.toml](../../../../packages/functions/auth/.cargo/config.toml).

## What

A linker override for the ARM64 Linux musl target.

## Why

Supports explicit cross-compilation without changing the default host target for local tests.

## How

Selects aarch64-linux-musl-gcc for that target. The build table does not set a default target; deployment invokes cargo-lambda explicitly.
