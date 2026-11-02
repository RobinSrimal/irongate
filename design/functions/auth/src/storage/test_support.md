# Unit-Test Backend

Source: [packages/functions/auth/src/storage/test_support.rs](../../../../../packages/functions/auth/src/storage/test_support.rs).

## What

An in-memory backend for storage-dependent unit tests.

## Why

Counter and transaction behaviour can be tested without AWS.

## How

A locked map stores JSON with expiry; CAS and transaction conditions are evaluated before committing staged writes. This backend models local storage behaviour but is not evidence of DynamoDB consistency or AWS SDK error semantics.
