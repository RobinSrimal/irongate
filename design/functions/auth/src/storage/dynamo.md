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

After existing TTL filtering, GetItem requires a string `value` containing valid JSON for every
returned physical item. Both Query paths also require string `pk` and `sk` attributes. Missing or
non-string values/keys and invalid JSON return fixed, sanitized `StorageError::DynamoDB` errors,
without SDK payloads, stored content or JSON decoder details. They are not interpreted as absence,
skipped rows or successful partial results. An absent item still returns None; expired items retain
the existing filtering behavior. This is not stricter TTL parsing or validation of typed record
fields. Typed stores validate their own bindings and state, including email ownership indexes whose
malformation must not hide unfinished deletion cleanup.

Transaction conditions stay attached to their write item. The private
[error classifier](dynamo_errors.md) distinguishes conditions, conflicts and operational failures
without copying service payloads into diagnostics. No application retry is added. Transactional Put
replaces both the JSON value and physical TTL, removing any prior TTL when expiry is None. There is
no value-only transaction update operation.
