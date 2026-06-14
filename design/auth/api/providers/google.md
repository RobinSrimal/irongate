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
issue OAuth authorization code
```

## Security Invariants

- Identity key is `issuer + sub`, not email.
- Validate ID token signature, issuer, audience, expiry, and nonce.
- Do not auto-link accounts by email.
- Store only minimal provider claims.
