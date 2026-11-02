# Password Reset Tests

Source: [packages/functions/auth/tests/password_reset.rs](../../../../packages/functions/auth/tests/password_reset.rs).

## What

Reset-key storage, generic unknown-email responses and password replacement.

## Why

Reset proofs must be one use and bound to an active verified subject.

## How

Tests inspect escaped messages, expected-subject checks, expiry and successful consumption/update using the memory backend. They do not demonstrate atomic proof consumption plus password replacement or credential-generation invalidation.
