# Storage Boundary

Source: [packages/functions/auth/src/storage/mod.rs](../../../../../packages/functions/auth/src/storage/mod.rs).

## What

Exports of the backend trait, DynamoDB implementation and transaction primitives.

## Why

Typed AuthStore is the runtime boundary while integration tests need backend injection.

## How

StorageAdapter remains public for fixtures. Runtime provider/route imports are prohibited by validate-store-boundary.mjs. The unit-test backend is cfg(test) only.
