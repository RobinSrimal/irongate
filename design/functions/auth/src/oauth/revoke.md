# Refresh Revocation

Source: [packages/functions/auth/src/oauth/revoke.rs](../../../../../packages/functions/auth/src/oauth/revoke.rs).

## What

Client-bound user-facing refresh-family revocation.

## Why

Logout invalidates renewable session state without revealing token existence.

## How

After form/client-ID parsing, source-first admission charges an aggregate source budget and a
configured-client/source budget before client authentication or refresh lookup. Unknown names share
structural None, not a reserved client name. Failed authentication consumes allowance. Valid clients
use the configured authentication method and HMAC token lookup. Unknown/foreign/revoked tokens produce
an idempotent success; non-refresh hints are ignored. Existing access JWTs remain valid until expiry;
the injected emitter attempts a typed event only for revoked/already-revoked results, with configured
client ID, HMAC digest and typed reason. Unknown/foreign tokens and rejected requests add no success
event. Audit output failure does not alter revocation or its response.
