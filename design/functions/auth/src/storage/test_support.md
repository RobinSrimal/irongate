# Unit-Test Backend

Source: [packages/functions/auth/src/storage/test_support.rs](../../../../../packages/functions/auth/src/storage/test_support.rs).

## What

An in-memory backend for storage-dependent unit tests.

## Why

Counter and transaction behaviour can be tested without AWS.

## How

A locked map stores JSON with expiry; CAS and transaction conditions are evaluated before writes.
Put replaces the whole value and TTL rather than merging fields. A source-local test checks omitted
fields disappear, renewed expiry is retained and None removes TTL. Conditions still treat logically
expired rows as absent, unlike DynamoDB's physical-item conditions; pagination also differs. This
backend is not evidence of DynamoDB consistency or AWS SDK error semantics.
