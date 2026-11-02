# Token Signing

Source: [packages/functions/auth/src/crypto/signing.rs](../../../../../packages/functions/auth/src/crypto/signing.rs).

## What

A common local/KMS ES256 signer, public JWKS construction and access-token verification.

## Why

First-party token semantics are independent of private-key custody.

## How

Local mode loads PKCS#8 material; KMS delegates signing. Both assemble JOSE signatures and expose one configured public key. Verification checks algorithm, kid, issuer, audience, expiry and access mode. The current Jwk field use_ serializes under that name, and there is no retained-key rotation set.
