# DynamoDB Counter Contracts

Source: [packages/functions/auth/tests/dynamodb_storage/rate_limits.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/rate_limits.rs).

## What

Shared admission cases and physical counter TTL checks against DynamoDB Local.

## Why

An in-memory counter does not establish actual conditional-write races or physical TTL attributes.

## How

Instantiates the six shared contracts through the existing loopback/dummy-credential/owned-table
fixture. Concurrent readers delegate CAS to DynamoDB; lost-response injection follows a real commit.
An extra case reads raw items to prove twice-window TTL on creation and rollover while the old item
remains present. All cases are ignored by default and require the explicit Local command in the
[test guide](../README.md). No scans, live tables, AWS profiles or waiting for TTL deletion are used.
