# Refresh Revocation

Source: [packages/functions/auth/src/oauth/revoke.rs](../../../../../packages/functions/auth/src/oauth/revoke.rs).

## What

Client-bound user-facing refresh-family revocation.

## Why

Logout invalidates renewable session state without revealing token existence.

## How

Accepts form data and the configured client authentication method, applies a client/source limit and performs HMAC lookup. Unknown/foreign/revoked tokens produce an idempotent success; non-refresh hints are ignored. Existing access JWTs remain valid until expiry; enabled audit records use a digest reference.
