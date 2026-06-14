# Signing

Target code: `packages/functions/auth/src/crypto/signing.rs`

## Owns

- Access token signing.
- Refresh token signing.
- JWKS public key generation or KMS key metadata exposure.

## Target Options

Initial simple mode:

```text
local ES256 keypair
private key stored outside generic AuthTable reads or encrypted at application level
```

Hardened mode:

```text
AWS KMS asymmetric signing
private key non-exportable
```

## Security Invariants

- Algorithm is explicit and fixed.
- Issuer and audience are set intentionally.
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
