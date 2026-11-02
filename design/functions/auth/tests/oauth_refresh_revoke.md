# Refresh And Revoke Tests

Source: [packages/functions/auth/tests/oauth_refresh_revoke.rs](../../../../packages/functions/auth/tests/oauth_refresh_revoke.rs).

## What

Refresh issuance, HMAC keys, transactional operation shape, rotation/reuse and client-bound revocation.

## Why

Refresh sessions must remain one-use and logout must not reveal token existence.

## How

Exercises store and router paths with TestStorage, including offline_access, audit opt-out and rate limits. Current tests do not prove DynamoDB TTL renewal, eventual-consistency behaviour or uncertain write outcomes.
