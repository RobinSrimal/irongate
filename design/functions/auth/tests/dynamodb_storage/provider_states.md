# DynamoDB Provider State Tests

Source: [packages/functions/auth/tests/dynamodb_storage/provider_states.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/provider_states.rs).

## What

Disposable Local runner for shared state contracts and a physical TTL check.

## Why

Real conditional deletes and physical expiry cannot be proved by memory-only tests.

## How

The existing loopback-only fixture creates/deletes a unique table for each shared case. Gates
delegate actual writes to DynamoStorage; no backend condition is mocked. A Local-only case reads
the raw item, verifies the browser HMAC and matching logical/physical expiry, checks that a wrong
provider retains the complete value/TTL, and confirms successful consumption deletes it.
Tests require an explicit local endpoint and are ignored in ordinary runs. They do not certify
deployed IAM, network outcomes or asynchronous TTL cleanup.
