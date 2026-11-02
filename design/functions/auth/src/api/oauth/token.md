# Token API Export

Source: [packages/functions/auth/src/api/oauth/token.rs](../../../../../../packages/functions/auth/src/api/oauth/token.rs).

## What

The router-facing re-export of the token handler.

## Why

The API namespace groups route entrypoints without duplicating protocol behaviour.

## How

Re-exports the implementation from `oauth/token.rs`. See the [protocol module](../../oauth/token.md) for validation, state and response details.
