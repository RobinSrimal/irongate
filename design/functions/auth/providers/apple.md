# Apple Identity Provider

Target code: `packages/functions/auth/src/providers/apple.rs`

## Owns

- Sign in with Apple configuration.
- Apple client-secret JWT generation.
- Apple authorization URL construction.
- Token exchange with Apple.
- Apple ID token validation.
- Mapping Apple claims to verified identity.
- Active account checks after identity mapping.

## Implementation Slices

Slice 10 implements Apple runtime configuration, Apple client-secret JWT generation, and authorization URL construction.

Slice 11 implements callback handling:

```text
receive Apple code and state
consume provider state
generate client-secret JWT
exchange code
validate ID token
map issuer + sub to internal subject
require active account
issue OAuth authorization code
```

## Security Invariants

- Validate issuer `https://appleid.apple.com`.
- Validate audience against configured Apple client ID.
- Validate signature, expiry, issued-at tolerance, and nonce.
- Apple private key comes from secrets.
- Apple client-secret JWTs are generated at runtime and are not stored.
- Identity key is Apple issuer plus `sub`.
- Do not assume email or name is always present.
- Disabled or deleted accounts cannot sign in.
