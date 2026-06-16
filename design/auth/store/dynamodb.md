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

When a transaction needs to guard a write or consume operation for one item, the condition must be attached to that same `Put`, `Update`, or `Delete` operation. Do not model same-item guards as a separate `ConditionCheck` plus a write/delete in the same DynamoDB transaction; AWS rejects transactions with multiple operations on one item.

## Security Invariants

- No runtime auth flow should require an unbounded scan.
- Expired records must be rejected at read time even before DynamoDB TTL deletes them.
- Conditional conflicts must be surfaced as domain errors.
- DynamoDB errors should not leak raw item values in logs.
- Generic writes are internal to the store and not exposed to auth routes.
- One-time consume and refresh rotation use conditional writes or transactions.
- One-time consumes use conditional deletes so replay protection is atomic and AWS-valid.

## Security Scan Coverage

The typed DynamoDB store is the main guardrail for the scan findings. It removes the generic write path that allowed expiry loss, removes route-controlled verification bypasses, and makes runtime scans visible as design violations.
