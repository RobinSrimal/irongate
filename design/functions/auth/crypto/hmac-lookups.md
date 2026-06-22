# HMAC Lookups

Target code: `packages/functions/auth/src/crypto/hmac.rs`

## Owns

- HMAC-SHA256 lookup digest generation.
- Input normalization for lookup secrets.

## Target Uses

- Authorization code lookup.
- Refresh token lookup.
- Email verification lookup.
- Password reset lookup.
- Provider state lookup.
- OAuth session lookup, when sessions are bearer-capable.
- Normalized email lookup for password users and rate limits.

## Digest Contract

```text
lookup_digest = base64url_no_pad(HMAC-SHA256(storage_lookup_secret, context || ":" || value))
```

The `context` string separates families so the same raw value cannot collide across record types:

```text
oauth_session
provider_state
auth_code
refresh_token
password_verify
password_reset
password_email
```

## Security Invariants

- HMAC secret comes from deployment secrets.
- Raw secret values are never logged.
- HMAC output is safe to use as DynamoDB key material.
- Secret rotation requires a planned compatibility window.
- The HMAC secret must not be stored in DynamoDB.
- Digest comparison should not expose raw inputs in errors or traces.

## Rotation Constraint

Rotating `storage_lookup_secret` affects lookup for active sessions, codes, refresh tokens, and password reset/verification secrets. Rotation needs one of:

- A compatibility window that checks both current and previous secrets.
- Forced invalidation of active one-time records and refresh tokens.
- A migration process that rewrites lookup keys without exposing raw secrets, if raw values are still available outside storage.
