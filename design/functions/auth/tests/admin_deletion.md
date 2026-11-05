# Deletion Tests

Source: [packages/functions/auth/tests/admin_deletion.rs](../../../../packages/functions/auth/tests/admin_deletion.rs).

## What

Account deletion, tombstones, reuse policies and the sanitized delete route.

## Why

Erasure must remove credential/contact data while preserving the account-state boundary.

## How

Fixtures complete email-first registration through the shared helper before adding reset proofs
and refresh sessions. Tests inspect persisted results and IAM-context rejection using the memory
backend; they do not establish whole-account atomicity or external provider revocation.

Deletion clears the pending registration binding alongside credential/contact data. Immediate
reuse creates a hashless, subjectless pending instance; completing its emailed proof with a new
password yields a different subject rather than reactivating the deleted account.

Seeded authorization codes carry their account's generation; deleting the account keeps those
credentials unusable without changing the existing deletion route contract.

Application-state fixtures explicitly install the shared [captured audit emitter](support/audit.md),
so tests do not write production stdout/stderr. Dedicated audit route suites assert emission itself.
