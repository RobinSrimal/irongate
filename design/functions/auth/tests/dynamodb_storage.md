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

Cases are ignored in ordinary Cargo runs and require DYNAMODB_LOCAL_ENDPOINT as documented in the
[test guide](README.md). They do not certify production consistency/IAM, asynchronous TTL cleanup,
every lifecycle race or full memory/DynamoDB equivalence.
