# Google Identity Provider

Target code: `packages/functions/auth/src/providers/google.rs`

## Owns

- Google OIDC configuration.
- Token exchange with Google.
- Google ID token validation.
- Mapping Google claims to verified identity.
- Active account checks after identity mapping.

## Security Invariants

- Validate issuer `https://accounts.google.com`.
- Validate audience against configured Google client ID.
- Validate signature, expiry, issued-at tolerance, and nonce.
- Identity key is Google issuer plus `sub`.
- Email is an attribute, not the primary identity key.
- Disabled or deleted accounts cannot sign in.
