# Discovery API Export

Source: [packages/functions/auth/src/api/oauth/discovery.rs](../../../../../../packages/functions/auth/src/api/oauth/discovery.rs).

## What

The router-facing re-export of the discovery handlers.

## Why

The API namespace groups route entrypoints without duplicating protocol behaviour.

## How

Re-exports the implementation from `oauth/well_known.rs`. See the [protocol module](../../oauth/well_known.md) for validation, state and response details.
