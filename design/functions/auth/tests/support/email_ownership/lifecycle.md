# Ownership Lifecycle Contracts

Source: [packages/functions/auth/tests/support/email_ownership/lifecycle.rs](../../../../../../packages/functions/auth/tests/support/email_ownership/lifecycle.rs).

## What

Shared deletion, retention, non-deletion lifecycle and cleanup-retry contracts.

## Why

Deleting an account must erase its email-owner subject linkage without losing the reverse index
before cleanup commits. An error cannot imply that the overall account deletion was rolled back.

## How

Accounts start through real registration completion. Disable/enable and a real password reset leave
owner/index values unchanged. Deletion under never, immediate and retention policies creates the
exact minimal tombstone using the persisted account deletion time, removes the index, and remains
stable on repetition.

The transaction gate pauses email cleanup after earlier account/identity/credential cleanup. Cases
inject pre-dispatch failure, lose a response after an actual commit, or independently invalidate
the owner/index guard. Exact reads distinguish unchanged rows from committed tombstones; unfinished
cleanup conservatively blocks a newly prepared registration. Retrying deletion completes cleanup
without resetting its deletion time. The deliberately corrupt index case restores only that test
fixture before its ordinary retry.

Separate malformed owner/index bindings and state cases fail operationally with sanitized errors,
preserving both ownership rows even though the account is already deleted. Repairing the fixture
allows retry. These are per-owner atomicity tests, not a claim of whole-account transactional deletion.
