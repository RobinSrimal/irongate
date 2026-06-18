# Userinfo Endpoint

Target code: `packages/functions/auth/src/api/oauth/userinfo.rs`

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
- Userinfo is not a general-purpose token introspection endpoint.

## Account Lifecycle Boundary

Disable/delete blocks userinfo responses immediately because this endpoint checks account status. It does not invalidate already-issued access tokens at resource APIs that validate JWTs locally. Those tokens remain valid until `exp`.

## Inputs

- `Authorization: Bearer <access_token>`

## Browser CORS

Browser clients may call this endpoint only from configured `allowed_origins`. The auth router returns exact CORS origins and does not return wildcard origins.

## Store Operations

- `require_active_account`

## Outputs

- Subject identifier.
- Subject type.
- Minimal properties allowed by the token and client.
- `email` and `email_verified` only when the access token grants `email` scope and the values are available.
- Profile claims only when the access token grants `profile` scope and the values are available.
