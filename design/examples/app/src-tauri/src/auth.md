# Native Auth Runtime

Source: [packages/examples/app/src-tauri/src/auth.rs](../../../../../packages/examples/app/src-tauri/src/auth.rs).

## What

Password/provider code flows, loopback callback, refresh/revoke and keychain access.

## Why

Refresh credentials stay in the Rust backend rather than browser storage.

## How

Uses an environment-selected issuer with a reserved placeholder default, the public app client and S256. Provider login opens the system browser and checks callback path/state; password login follows API redirects manually. keyring stores one refresh credential. Access/ID tokens cross IPC to React; refresh has no durable pending-rotation marker or native ID-token nonce validation.
