# Token Signing

## What

ES256 access/ID tokens and JWKS use the configured [TokenSigner](../src/crypto/signing.md), with
[local or KMS configuration](../src/config/signing.md). Refresh tokens are opaque random values, not
signed JWTs.

## Why

Local signing avoids a network signing call; KMS keeps private signing material non-exportable.
Both modes retain the same claim and JOSE signature format.

## How

Local mode resolves a PKCS#8 private key from a secret-backed environment reference and derives its
public key. KMS mode loads public metadata at startup and converts KMS DER signatures into 64-byte
JOSE signatures. Verification is local using cached public material.

The public JWKS contains the single configured key, never the private key. Automatic rotation with
overlapping verification keys is not implemented. The current serializer exposes the JWK usage
field as `use_`, rather than the standard `use`. Only ES256 is advertised; this is not a claim of
OpenID Provider certification. Signing keys are not stored in AuthTable.
