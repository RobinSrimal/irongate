# DynamoDB Adapter

Source: [packages/functions/auth/src/storage/dynamo.rs](../../../../../packages/functions/auth/src/storage/dynamo.rs).

## What

AWS SDK item encoding, exact-key operations, partition queries and transactions.

## Why

Auth record serialization and conditional expressions are centralized.

## How

Items contain pk/sk, JSON-string value and optional numeric expiry. GetItem and both Query paths
request strong consistency, including continuation pages. Existing expiry filtering and cursor
semantics remain unchanged; query_prefix collects all pages, not a cross-page snapshot. Strong reads
use more read capacity than eventual reads and do not make read-then-write flows atomic.

Transaction conditions stay attached to their write item. The private
[error classifier](dynamo_errors.md) distinguishes conditions, conflicts and operational failures
without copying service payloads into diagnostics. No application retry is added. Update still
changes value only, leaving physical TTL untouched.
