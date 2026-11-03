# Refresh Token Storage

## What

Digest-keyed refresh records, family state and subject/client indexes, implemented in
[src/store/refresh.rs](../src/store/refresh.md).

## Why

Rotation must replace a token once; logout and operator revocation use family state instead of a
table-wide scan.

## How

Initial issuance writes token, family and indexes transactionally with TTL and a matching active
account guard. It carries the consumed code's generation, not a newly read generation. Rotation
checks token/family subject, client and generation agreement, expiry and account activity. Its
transaction guards that account, marks the old token replaced and creates a same-generation successor.
Reuse revokes the family. User logout is client-bound and idempotent; subject revocation queries
that subject's index.

Password recovery invalidates old generations without enumerating these records. A generation
rejection is an invalid credential, not reuse. Operational/conflict failures remain distinct from
an explicit stale-account rejection; missing generation fields fail closed as storage errors.

Rotation writes complete conditional replacements. The family and successor/index records use the
accepted new deadline for both `expires_at` and DynamoDB `expiry` (Unix seconds). The old token and
old indexes retain their original deadlines rather than accumulating indefinite replay history.
Revocation preserves the current family's and current token's own expiry while setting their revoked
state. Account records do not receive TTL.

This is the existing sliding refresh-lifetime policy, including a shorter caller-supplied deadline;
there is no additional absolute family lifetime. Logical expiry still rejects credentials before
physical cleanup. Missing/expired historical records cannot guarantee replay detection forever.
Token rotation and subsequent JWT signing are separate steps, so an unsuccessful response does not
prove rotation was rolled back. Already-issued access JWTs expire independently. Code changes do
not recover family records already lost to cleanup or backfill existing deployed records.
