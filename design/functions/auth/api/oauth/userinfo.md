# Userinfo Endpoint

Implementation: `packages/functions/auth/src/api/oauth/userinfo.rs`

## Owns

- Validate bearer access tokens.
- Return stable subject claims.

## Security Invariants

- Verify issuer, audience, expiry, algorithm, and key ID.
- Verify the subject still references an active account before returning claims.
- Return only intended claims.
- Never return refresh token state, provider secrets, or internal storage keys.
- Userinfo is called with an access token, not an ID token.
- API authorization and row-level access control should use access-token claims.

## Account Lifecycle Boundary

Userinfo checks account status through a strongly consistent base-table read. It rejects inactive
records, but a concurrent lifecycle change after that read is not locked out. Independently
validated access JWTs remain valid at resource APIs until `exp`.

## Inputs

- `Authorization: Bearer <access_token>`

## Browser CORS

Browser clients may call this endpoint only from configured `allowed_origins`. The auth router returns exact CORS origins and does not return wildcard origins.

## Store Operations

- `is_active_account`

## Outputs

- Subject identifier.
- Subject type.
- `email` and `email_verified` only when the access token grants `email` scope and the values are available.

Email values come from the signed token, not a fresh identity lookup. The current endpoint does not
require the `openid` scope in addition to a valid access token.

The endpoint intentionally stays minimal. Resource APIs that need richer user profile data should
use their own application records keyed by the stable subject identifier.
