# Admin Crate

Source: [packages/functions/admin/Cargo.toml](../../../packages/functions/admin/Cargo.toml).

## What

The separate irongate-admin bootstrap manifest.

## Why

Admin deployment reuses auth code but has an independent binary and runtime permission boundary.

## How

Depends on the local irongate library as auth, the DynamoDB SDK and Lambda/Tokio/tracing. Release settings match the auth binary; sharing a dependency does not grant the admin Lambda provider secrets.
