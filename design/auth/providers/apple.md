# Apple Identity Provider

Target code: `packages/functions/auth/src/providers/apple.rs`

## Owns

- Sign in with Apple configuration.
- Apple client-secret JWT generation.
- Token exchange with Apple.
- Apple ID token validation.

## Security Invariants

- Validate issuer `https://appleid.apple.com`.
- Validate audience against configured Apple client ID.
- Validate signature, expiry, issued-at tolerance, and nonce.
- Apple private key comes from secrets.
- Identity key is Apple issuer plus `sub`.
- Do not assume email or name is always present.
