# DynamoDB Store

Target code: `packages/functions/auth/src/store/dynamo.rs`

## Owns

- AWS SDK DynamoDB calls.
- Item serialization and deserialization.
- Conditional expressions.
- Transactions.

## Target Behavior

The store should use exact-key operations for normal auth paths:

```text
GetItem
PutItem
UpdateItem
DeleteItem
TransactWriteItems
Query on bounded partitions
```

The route/provider layers should not receive generic `get`, `set`, `remove`, or `scan` primitives. They should call typed auth-store operations that encode replay, expiry, verification, and transaction rules.

## Security Invariants

- No runtime auth flow should require an unbounded scan.
- Expired records must be rejected at read time even before DynamoDB TTL deletes them.
- Conditional conflicts must be surfaced as domain errors.
- DynamoDB errors should not leak raw item values in logs.
- Generic writes are internal to the store and not exposed to auth routes.
- One-time consume and refresh rotation use conditional writes or transactions.

## Security Scan Coverage

The typed DynamoDB store is the main guardrail for the scan findings. It removes the generic write path that allowed expiry loss, removes route-controlled verification bypasses, and makes runtime scans visible as design violations.
