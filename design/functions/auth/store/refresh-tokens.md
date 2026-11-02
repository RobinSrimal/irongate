# Refresh Token Storage

## What

Digest-keyed refresh records, family state and subject/client indexes, implemented in
[src/store/refresh.rs](../src/store/refresh.md).

## Why

Rotation must replace a token once; logout and operator revocation use family state instead of a
table-wide scan.

## How

Initial issuance writes token, family and indexes transactionally with TTL. Rotation checks client,
expiry and account activity, conditionally marks the old token replaced and creates its successor.
Reuse revokes the family. User logout is client-bound and idempotent; subject revocation queries
that subject's index.

The current family Update advances logical `expires_at` but does not update the existing DynamoDB
`expiry` attribute. Thus physical cleanup can precede a renewed family's logical expiry. Token
rotation and subsequent JWT signing are separate steps, so an unsuccessful response does not prove
rotation was rolled back. Already-issued access JWTs expire independently.
