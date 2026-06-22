# Signing

Target code: `packages/functions/auth/src/crypto/signing.rs`

## Owns

- Access token signing.
- ID token signing.
- Refresh token signing.
- JWKS public key generation or KMS key metadata exposure.

## Target Options

Signing mode is deployment configuration:

```text
AUTH_SIGNING_MODE=local-es256
AUTH_SIGNING_MODE=kms-es256
```

Local ES256 mode:

```text
AUTH_SIGNING_MODE=local-es256
AUTH_SIGNING_KEY_ID=<kid>
AUTH_SIGNING_PRIVATE_KEY_SECRET=<secret ref>
AUTH_SIGNING_PUBLIC_KEY=<public key or derived from private key>
```

KMS ES256 mode:

```text
AUTH_SIGNING_MODE=kms-es256
AUTH_SIGNING_KEY_ID=<kid>
AUTH_SIGNING_KMS_KEY_ID=<kms key id or alias>
```

Developers can use local ES256 in dev and production if they prefer lower latency and cost. KMS ES256 is the hardened production mode because the private key is non-exportable and signing calls are auditable.

AWS KMS ECDSA signatures are returned in ASN.1 DER form. JWT ES256 requires the JOSE raw `r || s` signature format, so the KMS signer must convert signatures before assembling the JWT.

## OIDC Algorithm Compatibility

V1 targets practical OIDC client compatibility with ES256. Standard clients should read `id_token_signing_alg_values_supported` from discovery and verify through JWKS.

Strict OpenID Provider certification may require adding RS256 support. That is a separate signing-algorithm decision and should not be implied by the ES256-only v1 template.

## Security Invariants

- Algorithm is explicit and fixed.
- Issuer and audience are set intentionally.
- Access-token and ID-token audiences are not conflated.
- Discovery metadata must match the configured signing algorithm.
- KMS ECDSA signatures are converted to JOSE format before JWT serialization.
- Private signing key material is never returned through APIs.
- Key rotation has a documented overlap period.
- Raw JWT private keys should not be readable through ordinary AuthTable access.
- If DynamoDB stores signing metadata, separate public key metadata from private key material.

## Storage Target

Preferred hardened design:

```text
AWS KMS asymmetric signing key
private key is non-exportable
JWKS exposes only public key material
```

Acceptable transitional design:

```text
local ES256 signing
private key encrypted before storage
decrypt permission narrower than table read permission
```

Avoid:

```text
storing plaintext private_key_pem in AuthTable value
allowing ordinary human or tooling raw table reads to access signing private keys
```
