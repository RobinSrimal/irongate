# Password Login Tests

Source: [packages/functions/auth/tests/password_login.rs](../../../../packages/functions/auth/tests/password_login.rs).

## What

Typed session/code storage and verified password-login continuation.

## Why

Successful authentication creates a code, while wrong credentials must not consume the session.

## How

Uses local Argon2 hashes, active account fixtures and TestStorage. Checks code redirect, expiry metadata, HMAC keys and one-use session behaviour.

The code-store fixture creates an actual account and binds its generation. Password snapshot/reset
races are checked separately against DynamoDB Local, not inferred from memory success cases.
