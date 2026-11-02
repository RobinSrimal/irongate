# Authorize Sessions

Source: [packages/functions/auth/src/store/authorize_sessions.rs](../../../../../packages/functions/auth/src/store/authorize_sessions.rs).

## What

Typed creation, lookup and consumption of authorize-session records.

## Why

Login continuation preserves the validated client request without exposing raw keys in storage.

## How

Records are written with their expiry as TTL under an HMAC digest. Runtime expiry is checked and single-use take uses a conditional Delete against the previously read value.
