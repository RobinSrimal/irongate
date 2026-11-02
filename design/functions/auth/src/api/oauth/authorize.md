# Authorize API Export

Source: [packages/functions/auth/src/api/oauth/authorize.rs](../../../../../../packages/functions/auth/src/api/oauth/authorize.rs).

## What

The router-facing re-export of the authorize handler.

## Why

The API namespace groups route entrypoints without duplicating protocol behaviour.

## How

Re-exports the implementation from `oauth/authorize.rs`. See the [protocol module](../../oauth/authorize.md) for validation, state and response details.
