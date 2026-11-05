# DynamoDB Registration Contracts

Source: [packages/functions/auth/tests/dynamodb_storage/registration.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/registration.rs).

## What

The eight shared registration cases plus physical TTL and persistence checks on DynamoDB Local.

## Why

Real conditional transactions must reject stale registration snapshots without partial credential,
account or index writes. The memory runner cannot establish physical TTL retention/removal.

## How

The existing loopback-only `dynamo::run` harness creates and cleans up a uniquely owned table for
each case. Cases delegate transactions to `DynamoStorage`, including the gate's ordered conflicts
and deliberately lost responses. The Local-specific case checks verification JSON expiry against
physical TTL, changes an eligible pending snapshot before dispatch, and compares all eight raw
candidate rows after cancellation. A fresh completion consumes the proof and leaves the seven
activated records without TTL. No scans, deployed tables, live emails or TTL-deletion sleeps are
used. Tests are ignored unless explicitly enabled with `DYNAMODB_LOCAL_ENDPOINT`.
