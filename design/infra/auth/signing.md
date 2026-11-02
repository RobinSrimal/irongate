# Signing Resources

Source: [infra/auth/signing.ts](../../../infra/auth/signing.ts).

## What

Conditional KMS signing resources or a local-key SST secret.

## Why

Private-key custody is selectable without changing JWT algorithms.

## How

KMS mode creates an ECC_NIST_P256 SIGN_VERIFY key, stage alias and restricted permissions. Local mode resolves AuthSigningPrivateKey into the public Lambda environment. Exports signer environment and key ARN/mode; there is no automatic overlapping public-key rotation.
