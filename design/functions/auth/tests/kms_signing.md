# KMS Signer Tests

Source: [packages/functions/auth/tests/kms_signing.rs](../../../../packages/functions/auth/tests/kms_signing.rs).

## What

Signer digest calls, public JWKS, DER conversion and KMS-mode startup.

## Why

KMS/local mode must agree on JWT format without requiring test AWS access.

## How

Injects KMS operations and local cryptographic key material to validate the emitted token. Invalid DER and absence of local private-key requirements are checked; no real KMS permission or latency claim follows.
