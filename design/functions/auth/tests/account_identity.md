# Identity Reuse Tests

Source: [packages/functions/auth/tests/account_identity.rs](../../../../packages/functions/auth/tests/account_identity.rs).

## What

Deleted-identity reuse and generated subject separation.

## Why

A returning identity must not resurrect the previous account.

## How

Creates/deletes/reuses an identity through AuthStore and asserts a new subject using TestStorage.
