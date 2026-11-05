# Recovery Fixtures

Source: [packages/functions/auth/tests/support/recovery.rs](../../../../../packages/functions/auth/tests/support/recovery.rs).

## What

A product-neutral verified password-account fixture for memory and DynamoDB Local recovery tests.

## Why

Recovery tests must start from eligible identities and exercise real password verification and
credential operations, not just compare copied record fields.

## How

Uses configured test runtime, the shared email-only registration/proof-plus-new-password helper,
an in-process mailbox and typed store operations. Imports `crate::support::registration` so the
registration source is included only once per test crate. Every recovery consumer, including
`dynamodb_storage`, declares the shared `support` module; recovery still uses caller-supplied storage.
Helpers request/complete resets, login with authorize sessions, inspect generation,
verify the actual hash and create/rotate refreshes. URL parsing extracts proof tokens in memory;
fixture secrets are not printed. It uses caller-supplied storage so local tests reach the actual
adapter. No provider, mail service or deployed credentials are used.
