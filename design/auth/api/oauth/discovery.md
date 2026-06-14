# Discovery And JWKS

Target code: `packages/functions/auth/src/api/oauth/discovery.rs`

## Owns

- OAuth authorization server metadata.
- JWKS response for public verification keys.

## Security Invariants

- Metadata issuer must match the configured public issuer URL.
- JWKS must expose public key material only.
- Private signing keys must never be serialized through this endpoint.
- Algorithms should be explicit and narrow.

## Store Operations

- `get_public_signing_keys` or KMS-backed equivalent metadata lookup.
