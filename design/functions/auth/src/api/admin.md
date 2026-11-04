# Admin HTTP API

Source: [packages/functions/auth/src/api/admin.rs](../../../../../packages/functions/auth/src/api/admin.rs).

## What

IAM-context-guarded account lookup, disable, enable, delete and session revocation.

## Why

Operator actions are isolated from public authentication and return sanitized lifecycle data.

## How

The router requires the existing API Gateway IAM authorizer context and inserts a trusted actor
extension using nonempty user ARN, caller ID, then user ID. Headers/body cannot set that actor.
Handlers call AuthStore, clear reset state and revoke families as appropriate. Confirmed account
records supply the target subject for typed audit events; cleanup counts come from store results.
AdminAppState holds the injected emitter, separate from storage. Emission failures never alter the
HTTP result. Denied/missing/conflicting actions do not emit success events.

Responses contain status/timestamps/counts instead of raw records. Cleanup and account-state
transitions are multiple operations, not one rollback-capable transaction; partial cleanup failure
can leave changes without a completed-action event.
