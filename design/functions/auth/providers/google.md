# Google Identity Provider

Implementation: `packages/functions/auth/src/providers/google.rs`

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
- Require a nonblank Google ID-token `kid` and match it exactly to the selected RSA key.
- Reject explicit JWK alg other than RS256 or use other than sig; absent optional alg/use is valid.
- Cache Google JWKS in warm Lambda memory for a bounded time.
- Refetch Google JWKS once when the token `kid` is not present in the current key set.
- Identity key is Google issuer plus `sub`.
- Email is an attribute, not the primary identity key.
- Disabled or deleted accounts cannot sign in.

The MissingKey callback category also includes absent/blank IDs and allows at most one forced
refresh. Signature, claim and explicit key-metadata failures do not trigger that retry. Failed
validation creates no account/identity/code, though provider state and authorize session have
already been consumed. These are current web-code-flow checks, not native presenter or nbf policy.
