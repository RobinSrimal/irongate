# Refresh Families

Source: [packages/functions/auth/src/store/refresh.rs](../../../../../packages/functions/auth/src/store/refresh.rs).

## What

Issuance, rotation, reuse detection and family/subject revocation.

## Why

Long-lived sessions need single-use rotation without table scans.

## How

Issuance writes token, family and indexes transactionally. Rotation conditionally replaces the old
token and family together with successor/index creation. The old token retains its original expiry;
the family, successor and new indexes receive the caller's new deadline in both `expires_at` and
physical TTL. Existing old indexes retain their deadlines. No maximum-session-age policy is added.

Rotation rejects wrong clients, expired/revoked state and inactive accounts. Revocation compares
previous values on full-record puts and preserves the family's and current token's own expiries.
Subject cleanup queries indexed records; family revocation blocks descendants. Runtime expiry is
independent of asynchronous TTL deletion. Account checks and JWT signing remain separate operations,
and expired replay records do not provide indefinite reuse detection.
