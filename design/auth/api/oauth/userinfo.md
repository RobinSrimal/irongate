# Userinfo Endpoint

Target code: `packages/functions/auth/src/api/oauth/userinfo.rs`

## Owns

- Validate bearer access tokens.
- Return stable subject claims.

## Security Invariants

- Verify issuer, audience, expiry, algorithm, and key ID.
- Return only intended claims.
- Never return refresh token state, provider secrets, or internal storage keys.

## Inputs

- `Authorization: Bearer <access_token>`

## Outputs

- Subject identifier.
- Subject type.
- Minimal properties allowed by the token and client.
