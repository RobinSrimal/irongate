# DynamoDB Confirmation Contracts

Source: [packages/functions/auth/tests/dynamodb_storage/sign_in_confirmation.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/sign_in_confirmation.rs).

## What

Explicitly enabled confirmation contracts on disposable DynamoDB Local tables.

## Why

Actual transaction cancellation and physical TTL attributes need the real storage adapter.

## How

Runs the shared proof/snapshot/expiry/race suite with DynamoStorage. A Local-only case checks pending
TTL equals logical expiry, successful completion removes the proof and adds code TTL, and activated
account/identity/ownership rows remain persistent. A collision with an existing usable code preserves
all physical rows and TTL after confirmed cancellation. Fixtures use loopback-only endpoints, dummy
credentials and unique tables; ordinary Cargo runs ignore these cases. No deployed tables or TTL
cleanup waits are used. See the [test guide](../README.md).
