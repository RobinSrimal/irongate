# Google Identity Provider

Target code: `packages/functions/auth/src/providers/google.rs`

## Owns

- Google OIDC configuration.
- Token exchange with Google.
- Google ID token validation.
- Google JWKS fetch, warm-Lambda caching, and stale-key refetch.
- Mapping Google claims to verified identity.
- Active account checks after identity mapping.

## Security Invariants

- Validate issuer `https://accounts.google.com`.
- Validate audience against configured Google client ID.
- Validate signature, expiry, issued-at tolerance, and nonce.
- Require the Google ID token header to contain a `kid`.
- Cache Google JWKS in warm Lambda memory for a bounded time.
- Refetch Google JWKS once when the token `kid` is not present in the current key set.
- Identity key is Google issuer plus `sub`.
- Email is an attribute, not the primary identity key.
- Disabled or deleted accounts cannot sign in.
