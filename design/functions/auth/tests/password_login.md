# Password Login Tests

Source: [packages/functions/auth/tests/password_login.rs](../../../../packages/functions/auth/tests/password_login.rs).

## What

Typed session/code storage and verified password-login continuation.

## Why

Successful authentication creates a code, while wrong credentials must not consume the session.

## How

Uses the shared email-first registration fixture and TestStorage. The fixture completes an emailed
proof with the chosen password before login; it does not seed a hash or bypass account activation.
Checks code redirect, expiry metadata, HMAC keys and one-use session behaviour.

The code-store fixture creates an actual account and binds its generation. Password snapshot/reset
races are checked separately against DynamoDB Local, not inferred from memory success cases.
