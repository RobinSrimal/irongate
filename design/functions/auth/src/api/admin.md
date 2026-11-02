# Admin HTTP API

Source: [packages/functions/auth/src/api/admin.rs](../../../../../packages/functions/auth/src/api/admin.rs).

## What

IAM-context-guarded account lookup, disable, enable, delete and session revocation.

## Why

Operator actions are isolated from public authentication and return sanitized lifecycle data.

## How

The router requires API Gateway IAM authorizer context. Handlers call AuthStore, clear reset state and revoke families as appropriate, returning status/timestamps/counts instead of raw records. Audit failures are best effort. Cleanup and account-state transitions are multiple operations, not a single rollback-capable transaction.
