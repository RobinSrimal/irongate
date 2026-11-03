# Refresh And Revoke Tests

Source: [packages/functions/auth/tests/oauth_refresh_revoke.rs](../../../../packages/functions/auth/tests/oauth_refresh_revoke.rs).

## What

Refresh issuance, HMAC keys, rotation/reuse and client-bound revocation.

## Why

Refresh sessions must remain one-use and logout must not reveal token existence.

## How

Exercises store and router paths with TestStorage, including offline_access, audit opt-out and rate
limits. A replacement contract checks that conditional Put drops omitted fields in the integration
fixture. Transaction call counts are not used as atomicity evidence; the
[local refresh suite](dynamodb_storage/refresh.md) checks rollback and TTL through the real adapter.
These memory tests do not certify production consistency or uncertain write outcomes.

A completed password reset leaves old code/refresh exchanges returning invalid_grant without
token responses or replay-family revocation. Its previously issued access JWT still verifies.
Code/refresh fixtures use required generation bindings; the real local suite checks atomicity.
