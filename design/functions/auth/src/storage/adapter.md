# Backend Contract

Source: [packages/functions/auth/src/storage/adapter.rs](../../../../../packages/functions/auth/src/storage/adapter.rs).

## What

Async get/set/remove/query/CAS/transaction methods and transaction conditions.

## Why

AuthStore can exercise the same persistence calls against DynamoDB and test fixtures.

## How

Operations use key parts and JSON records. Put carries explicit expiry; Update replaces value without an expiry parameter. Conditions include existence and attribute equality. Arc forwards calls to the underlying backend.
