# Apple Provider API

Target code: `packages/functions/auth/src/api/providers/apple.rs`

## Owns

- Start Sign in with Apple.
- Handle Apple callback.
- Call Apple-specific OIDC validation and client-secret generation.

## Implemented Boundaries

Slice 10 implements the API-only start route:

```text
GET /apple/authorize?session=<raw-authorize-session-key>
```

The route validates the typed authorize session, stores HMAC-keyed provider state, and redirects to Apple with `response_mode=form_post`, `state`, `nonce`, and PKCE.

Slice 11 implements the API-only callback route:

```text
POST /apple/callback
```

The callback consumes provider state and authorize session, exchanges the Apple code with a generated client-secret JWT, validates the Apple ID token, maps `issuer + sub` to an internal subject, and issues an internal OAuth authorization code.

## Target Flow

```text
create provider state and nonce
redirect to Apple
receive code and state
consume provider state
generate Apple client-secret JWT
exchange code
validate ID token
map issuer + sub to internal subject
require active account
issue OAuth authorization code
```

## Security Invariants

- Treat Apple as first-class OIDC, not generic OAuth2.
- Validate ID token signature, issuer, audience, expiry, and nonce.
- Apple private key material must come from secrets, not DynamoDB.
- Do not rely on Apple email as the canonical identity.
- Disabled or deleted accounts cannot receive an OAuth authorization code.
