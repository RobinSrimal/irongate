# Password Registration Tests

Source: [packages/functions/auth/tests/password_registration.rs](../../../../packages/functions/auth/tests/password_registration.rs).

## What

Password policy/hash, email normalization/rendering, delivery boundary and verification.

## Why

A pending signup must not issue auth tokens before email proof.

## How

Uses a captured sender and memory store to inspect pending users, escaped messages, single-use/expired proofs and persisted subjects after verification. It does not send real Resend mail.
