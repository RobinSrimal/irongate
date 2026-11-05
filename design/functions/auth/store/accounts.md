# Accounts

## What

Persisted generated subjects and lifecycle status in
[records](../src/store/records.md), managed by [AuthStore](../src/store/mod.md).

## Why

An account ID is independent of mutable email or external provider claims.

## How

Google [provider issuance](identities.md) creates the account, identity, reverse index and
authorization code together. A new Apple identity requires signed verified email and either guards
an active owner/account/index or creates fresh account/ownership state with its identity/index/code.
Returning issuance retains the complete active-account snapshot and
generation from preparation, then checks that exact value while updating the identity and creating
the code; it does not adopt a newer generation. Password [registration](../src/store/registration.md)
also creates its account within atomic proof/credential/email-owner completion, with the same fresh
subject in the credential, identity, owner and all reverse indexes. Pending registration creates
neither account nor ownership. An active owner blocks completion without account linking or repair.

Lifecycle writes compare the previously read record, and deleted accounts are terminal.
Account records retain subject, status, created_at,
optional disabled_at/deleted_at and required `credential_generation`. Every new account initializes
it to zero; atomic password recovery advances it with checked increment. Lifecycle replacements
preserve the counter and cannot overwrite a newer generation using a stale snapshot. It remains
internal and is omitted from sanitized admin responses. There is no separate accounts.rs module.
See [lifecycle](../core/account-lifecycle.md) for multi-step cleanup and concurrency limitations.

Delete performs subject-indexed email cleanup after existing credential/session cleanup and before
success. Each owner transition clears its subject while deleting the exact reverse index; guarded
stale-index removal leaves newer owners untouched. Errors are visible and the existing delete
action is retriable; earlier account changes remain committed. Disable/enable/reset leave ownership
intact. Existing identities never change accounts from later email claims, and no accounts are
merged or backfilled. Login/recovery does not require ownership rows.
