# Refresh Token Store

Target code: `packages/functions/auth/src/store/refresh.rs`

## Owns

- Refresh token record creation.
- Atomic refresh rotation.
- Reuse detection.
- Subject/client revocation indexes.

## Target Records

```text
refresh:<digest>
refresh_by_subject:<subject> / <digest>
refresh_by_client:<client_id> / <digest>
```

All records use the refresh token expiry as TTL.

## Security Invariants

- Refresh token lookup uses HMAC digest.
- Rotation is atomic.
- The old token is marked replaced when the new token is created.
- Reuse of a replaced or revoked token is detected.
- Revocation does not scan the entire refresh-token family.
