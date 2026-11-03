# DynamoDB Local Transactions

Source: [packages/functions/auth/tests/dynamodb_storage.rs](../../../../packages/functions/auth/tests/dynamodb_storage.rs).

## What

Explicitly enabled integration tests against disposable DynamoDB Local tables.

## Why

The real adapter's conditional writes and confirmed cancellation rollback cannot be established
by a success-only HTTP stub or an in-memory fake.

## How

Exercises conditional replacement, rollback across multiple items, duplicate-item rejection and
authorization-code consumption by two readers of the same record. The stale consumer commits after
the winner deterministically and receives the existing typed rejection, not a backend error. Cleanup
is checked after both success and panic while another fixture table remains intact.

Cases are ignored in ordinary Cargo runs and require DYNAMODB_LOCAL_ENDPOINT as documented in the
[test guide](README.md). They do not certify production consistency/IAM, every race, refresh expiry
or full memory/DynamoDB equivalence.
