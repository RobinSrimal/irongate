# Refresh Families

Source: [packages/functions/auth/src/store/refresh.rs](../../../../../packages/functions/auth/src/store/refresh.rs).

## What

Issuance, rotation, reuse detection and family/subject revocation.

## Why

Long-lived sessions need single-use rotation without table scans.

## How

Issuance writes token, family and indexes transactionally, guarded by the active account at the
consumed code's generation. Rotation conditionally replaces the old
token and family together with successor/index creation. The old token retains its original expiry;
the family, successor and new indexes receive the caller's new deadline in both `expires_at` and
physical TTL. Existing old indexes retain their deadlines. No maximum-session-age policy is added.

Rotation requires token/family subject, client and generation agreement and includes a matching
active-account guard in its transaction. Successors carry the original generation. Generation
rejection is an invalid credential, not reuse; confirmed condition races are rechecked for account
invalidation while other storage failures remain operational. Revocation compares
previous values on full-record puts and preserves the family's and current token's own expiries.
Subject cleanup queries indexed records; family revocation blocks descendants. Runtime expiry is
independent of asynchronous TTL deletion. JWT signing remains outside the storage transaction,
and expired replay records do not provide indefinite reuse detection.
