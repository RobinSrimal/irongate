# Backend Contract

Source: [packages/functions/auth/src/storage/adapter.rs](../../../../../packages/functions/auth/src/storage/adapter.rs).

## What

Async get/set/remove/query/CAS/transaction methods and transaction conditions.

## Why

AuthStore can exercise the same persistence calls against DynamoDB and test fixtures.

## How

Operations use key parts and JSON records. Put carries explicit expiry; Update replaces value without an expiry parameter. Conditions include existence and attribute equality. Arc forwards calls to the underlying backend.

DynamoDB base-table reads request strong consistency. A read does not lock a record or make a
subsequent write atomic, and multiple query pages are not a snapshot. Transactions distinguish
confirmed condition failures, transaction conflicts and operational failures; transport errors can
leave the commit outcome unknown.
