# Cookies

Target code: `packages/functions/auth/src/crypto/cookies.rs`

## Owns

- Secure cookie construction for browser flow state.
- Cookie parsing helpers.

## Target Behavior

Cookies should store only opaque identifiers, not full auth state.

Irongate core does not use cookies for bearer token storage. The web BFF example uses HttpOnly
session cookies owned by the Worker, not JavaScript-readable access or refresh-token cookies.

## Security Invariants

- `HttpOnly` for session cookies.
- `Secure` in deployed HTTPS environments.
- `SameSite=Lax` unless a specific flow requires otherwise.
- Short max-age matching the backing session TTL.
- Cookie values must be random and unguessable.
- JavaScript-readable cookies must not store access tokens, refresh tokens, or ID tokens.
