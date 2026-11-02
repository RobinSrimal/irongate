# Userinfo API Export

Source: [packages/functions/auth/src/api/oauth/userinfo.rs](../../../../../../packages/functions/auth/src/api/oauth/userinfo.rs).

## What

The router-facing re-export of the userinfo handler.

## Why

The API namespace groups route entrypoints without duplicating protocol behaviour.

## How

Re-exports the implementation from `oauth/userinfo.rs`. See the [protocol module](../../oauth/userinfo.md) for validation, state and response details.
