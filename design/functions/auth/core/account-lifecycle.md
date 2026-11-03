# Account Lifecycle

## What

Accounts are active, disabled or deleted. Implementation lives in
[AuthStore](../src/store/mod.md), [records](../src/store/records.md) and
[admin handlers](../src/api/admin.md), not a separate core/account_lifecycle module.

## Why

Operators can block future authentication, restore disabled accounts or remove auth-owned contact
and credential material without raw table access.

## How

Disable changes status and the admin handler revokes refresh families and clears reset proofs.
Enable accepts disabled or already-active accounts, rejects deleted accounts, revokes sessions and
clears reset proofs. Delete marks the account deleted before indexed identity/password/proof/session
cleanup. These are multiple operations; a partial failure is not a whole-operation rollback.

Tombstones remove password hashes/contact properties and identity subject links, retaining digest,
status and lifecycle timestamps. Existing record structures also retain creation/last-seen metadata;
they are not restricted to a three-field account object. Subject IDs are never reused. Configured
identity reuse controls whether a new account can claim a deleted identity.

Atomic password recovery increments the account's internal credential generation. Lifecycle
replacements preserve it and compare the complete previous record, so a stale disable/delete/enable
write cannot roll the counter backwards. A failed lifecycle action remains visible; cleanup and
status changes still span separate operations. Lifecycle actions do not independently increment
generation. Sanitized responses do not include it.

Code creation/consumption and refresh creation/rotation guard an active account at their original
generation. Password login additionally guards its verified hash snapshot. JWT signing is outside
those transactions; an already-authorized request may finish after a lifecycle/recovery commit.
Already-issued self-contained JWTs remain valid at independent resource servers until expiry;
userinfo performs its own account-status check.
