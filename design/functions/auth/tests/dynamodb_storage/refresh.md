# Local Refresh Transactions

Source: [packages/functions/auth/tests/dynamodb_storage/refresh.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/refresh.rs).

## What

Disposable DynamoDB Local coverage for refresh issuance, rotation, revocation and runtime expiry.

## Why

Logical record assertions and transaction counts do not prove physical TTL renewal or all-or-nothing
failure under stale writers.

## How

Runs under the ignored dynamodb_storage target using fixture-owned tables. Raw consistent reads
compare logical and physical deadlines; bounded exact-partition queries capture state without scans.
Initial issuance is challenged by occupying its generated family key before commit, forcing a real
NotExists cancellation without partial writes. Successful issuance checks all four records.

Rotation checks family/successor/index renewal, old-token/index retention, a shorter accepted new
deadline, client-bound logout, indexed revocation and idempotence. A transaction gate lets another
rotation or revocation commit after the losing request has read state. The loser reaches DynamoDB,
fails its condition and leaves the winner's values/TTLs unchanged with no orphan records.
Replay still revokes the winner's family. Expired token/family payloads with future physical TTL
remain rejected independently of background cleanup.

Uses distinct future timestamps, not sleeps or a production test clock. Snapshots avoid formatting
tokens or digests in assertion failures. These cases do not prove the AWS TTL deletion schedule or
couple account lifecycle checks and signing atomically to rotation.
