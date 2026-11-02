# KMS Signer

Source: [packages/functions/auth/src/crypto/kms_signing.rs](../../../../../packages/functions/auth/src/crypto/kms_signing.rs).

## What

AWS KMS operations behind the TokenSigner facade.

## Why

KMS keeps private signing material outside the Lambda process.

## How

Startup loads and checks an asymmetric P-256 signing key and caches its public material. Signing calls KMS and converts DER signatures to the fixed-width JOSE representation. Verification uses the cached public key locally; injectable operations support deterministic tests.
