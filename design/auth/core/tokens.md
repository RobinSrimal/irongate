# Tokens

Target code: `packages/functions/auth/src/core/tokens.rs`

## Owns

- Access token claim shape.
- Refresh token metadata shape.
- Token TTL policy.
- Token family rules.

## Target Behavior

Access tokens are signed JWTs and are not persisted.

Refresh tokens are persisted only by HMAC lookup digest and rotate on every use.

## Security Invariants

- Access token TTL is short.
- Refresh token TTL is bounded.
- Refresh token reuse is detectable.
- Refresh token rotation is atomic in the store.
- Token claims are minimal and predictable.
