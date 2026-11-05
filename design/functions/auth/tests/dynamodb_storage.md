# DynamoDB Local Transactions

Source: [packages/functions/auth/tests/dynamodb_storage.rs](../../../../packages/functions/auth/tests/dynamodb_storage.rs).

## What

Explicitly enabled integration tests against disposable DynamoDB Local tables.

## Why

The real adapter's conditional writes and confirmed cancellation rollback cannot be established
by a success-only HTTP stub or an in-memory fake.

## How

Exercises complete value/TTL replacement, stale-condition rollback across multiple items,
duplicate-item rejection and
authorization-code consumption by two readers of the same record. The stale consumer commits after
the winner deterministically and receives the existing typed rejection, not a backend error. Cleanup
is checked after both success and panic while another fixture table remains intact.

Raw item reads prove TTL renewal/removal rather than inferring it from logical JSON values. Account
disable/enable/delete remain persistent, and deleted accounts cannot be enabled. The
[refresh child module](dynamodb_storage/refresh.md) adds renewal, revocation and stale-writer tests.
Code fixtures have real generation-bound accounts. The [recovery module](dynamodb_storage/recovery.md)
adds atomic reset, rejection and uncertain-outcome checks; its
[race module](dynamodb_storage/recovery_races.md) orders competing credential/lifecycle writers
through actual DynamoDB conditions.

The [rate-limit module](dynamodb_storage/rate_limits.md) runs the same admission contracts as memory
storage, plus raw counter TTL checks. Gates synchronize actual reads before competing conditional
writes; controlled faults cover pre-dispatch failure and a successful write with a lost response.

The [registration module](dynamodb_storage/registration.md) runs eight shared memory/Local
contracts, including all eight completion conditions, competing password winners, issuance races,
stale/recreated bindings, controlled expiry and uncertain outcomes. A Local-specific case verifies
physical proof TTL, raw rollback and persistent activated rows. Its shared login checks use the
crate-root recovery fixture. The target declares `support` so recovery reuses the single
`support::registration` module; importing those helpers does not replace the Local storage adapter.

The [ownership module](dynamodb_storage/email_ownership.md) runs shared owner-state, retention,
deletion/reuse and uncertain-outcome contracts on real transactions. Local-only cases inspect
physical owner/index/tombstone persistence and corrupt physical record values through raw SDK
writes. Exact-key reads and both query methods must reject malformed values without hiding
unfinished ownership cleanup; repairing the fixture permits a safe deletion retry.

The [provider-issuance module](dynamodb_storage/provider_issuance.md) runs eight shared contracts
for atomic new/returning/deleted identity and authorization-code writes. It covers usable codes,
every condition independently, exact orphan checks, competing callbacks and lifecycle writers in
both orders, session expiry and uncertain outcomes. A Local-only case checks code TTL, persistent
identity/account/index records and raw rollback after code collision. The `support` module also
supplies a real verified password fixture for no-email-linking checks.

Cases are ignored in ordinary Cargo runs and require DYNAMODB_LOCAL_ENDPOINT as documented in the
[test guide](README.md). They do not certify production consistency/IAM, asynchronous TTL cleanup,
every lifecycle race or full memory/DynamoDB equivalence.
