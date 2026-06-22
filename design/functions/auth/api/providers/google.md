# Google Provider API

Target code: `packages/functions/auth/src/api/providers/google.rs`

## Owns

- Start Google OIDC login.
- Handle Google callback.
- Call provider validation code.

## Target Flow

```text
create provider state and nonce
redirect to Google
receive code and state
consume provider state
exchange code
validate ID token
map issuer + sub to internal subject
require active account
issue OAuth authorization code
```

## Security Invariants

- Identity key is `issuer + sub`, not email.
- Validate ID token signature, issuer, audience, expiry, and nonce.
- Do not auto-link accounts by email.
- Store only minimal provider claims.
- Rate-limit provider-start by provider, authorize-session lookup digest, and trusted API Gateway source identity.
- Provider-start rate-limit keys must not contain raw authorize session keys.
- Disabled or deleted accounts cannot receive an OAuth authorization code.
