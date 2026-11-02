# Revoke API Export

Source: [packages/functions/auth/src/api/oauth/revoke.rs](../../../../../../packages/functions/auth/src/api/oauth/revoke.rs).

## What

The router-facing re-export of the revoke handler.

## Why

The API namespace groups route entrypoints without duplicating protocol behaviour.

## How

Re-exports the implementation from `oauth/revoke.rs`. See the [protocol module](../../oauth/revoke.md) for validation, state and response details.
