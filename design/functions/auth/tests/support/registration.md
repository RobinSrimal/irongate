# Registration Fixtures

Source: [packages/functions/auth/tests/support/registration.rs](../../../../../packages/functions/auth/tests/support/registration.rs).

## What

A captured mailbox and shared helper for creating a verified password account through email-first
registration and deliberate proof completion.

## Why

Login, reset, lifecycle and token tests need eligible password accounts without bypassing the
registration binding, credential generation or atomic completion rules.

## How

`Mailbox` implements the production email-sender trait, parses the configured link with `url::Url`
and keeps tokens only in process memory. Tests can inspect the latest proof and delivery count;
no real mail service or credentials are used.

`register_verified_user` submits email only, then completes the delivered proof with the chosen
password and returns its subject. Callers provide the runtime and store so the same source works
with memory storage or the actual disposable Local adapter. It neither seeds hashes directly nor
creates an OAuth login session. The shared `support` module includes this file once per test crate;
recovery and other consumers reuse `crate::support::registration` rather than loading a second copy.
