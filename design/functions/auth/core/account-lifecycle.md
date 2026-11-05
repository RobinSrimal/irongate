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
cleanup, then cleans email ownership before returning success. These are multiple operations; a
partial failure is not a whole-operation rollback. Disable/enable leave email ownership intact.

Tombstones remove password hashes/contact properties and identity subject links, retaining digest,
status and lifecycle timestamps. Existing record structures also retain creation/last-seen metadata;
they are not restricted to a three-field account object. Subject IDs are never reused. Configured
identity reuse controls whether a new account can claim a deleted identity.

Email-only pending registrations have no account subject, password hash or email ownership.
Their `registration_id` binds each proof to one instance.
[Registration completion](../src/store/registration.md) validates
identity and email-owner eligibility and atomically consumes the proof while creating the verified
credential, new generation-zero account, password identity, email owner and three subject indexes.
An active password identity or email owner cannot be replaced through registration, including
when associated with a disabled account. An active owner with a missing/deleted account still blocks
completion; registration does not repair it or attach credentials to its subject.

Password tombstoning also clears the pending registration ID. Allowed deleted-user recreation
creates a fresh pending ID, and completion checks and conditionally replaces the exact eligible
identity and email-owner tombstones. Both must allow reuse, including stored reuse permission even
under immediate policy. Earlier proofs cannot activate that instance or revive a previous subject.
There is no automatic cross-provider linking. The clean registration contract invalidates old
pending records/proofs without migration/backfill. Existing login/recovery does not require
ownership records, but historical accounts are not certified as indexed. Acceptance uses fresh
disposable data.

[Email ownership cleanup](../src/store/email_ownership.md) queries only the requested subject's
index partition, following all production query pages and validating bindings. For each owner still
pointing to that subject, one transaction replaces its exact value with a subjectless tombstone and
deletes the exact reverse index. It uses the persisted account deletion timestamp and existing reuse
policy. Owner/index/tombstone records contain no raw email and have no TTL.

Absent, deleted or different-subject owners are not rewritten. Stale-index deletion is still guarded
by owner absence or exact equality. Errors propagate, leaving the existing delete action available
for retry; no receipt or compensation is added. Old deletion retries cannot tombstone a newer owner
or delete its new subject index. A partially completed deletion may block new registration until
cleanup succeeds. Password reset leaves ownership unchanged.

Atomic password recovery increments the account's internal credential generation. Lifecycle
replacements preserve it and compare the complete previous record, so a stale disable/delete/enable
write cannot roll the counter backwards. A failed lifecycle action remains visible; cleanup and
status changes still span separate operations. Lifecycle actions do not independently increment
generation. Sanitized responses do not include it.

[Provider issuance](../store/identities.md) pins the returning identity and complete active-account
snapshot, including credential generation, during preparation. Its final transaction checks that
account, conditionally replaces the same identity and creates the code together. It does not reread
or retry against a newer generation. New or reusable deleted identities create a fresh generation-zero
account, identity, subject index and code in one transaction; deleted reuse never revives the old subject.

These guards reject state/generation differences present at commit, not every intervening lifecycle
action. A disable-then-enable sequence that restores the same complete account value is not detected
as history; there is no new lifecycle epoch. If issuance wins a race, later token-exchange guards
still reject an inactive account or changed generation. Earlier provider-state/session consumption
is outside issuance, so a rejected callback requires a fresh sign-in, not restoration of that session.

Code creation/consumption and refresh creation/rotation guard an active account at their original
generation. Password login additionally guards its verified hash snapshot. JWT signing is outside
those transactions; an already-authorized request may finish after a lifecycle/recovery commit.
Already-issued self-contained JWTs remain valid at independent resource servers until expiry;
userinfo performs its own account-status check.
