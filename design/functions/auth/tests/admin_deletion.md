# Deletion Tests

Source: [packages/functions/auth/tests/admin_deletion.rs](../../../../packages/functions/auth/tests/admin_deletion.rs).

## What

Account deletion, tombstones, reuse policies and the sanitized delete route.

## Why

Erasure must remove credential/contact data while preserving the account-state boundary.

## How

Fixtures include password proofs, identities and refresh sessions. Tests inspect persisted results and IAM-context rejection using the memory backend; they do not establish whole-account atomicity or external provider revocation.
