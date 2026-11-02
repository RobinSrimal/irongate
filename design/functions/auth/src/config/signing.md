# Signing Configuration

Source: [packages/functions/auth/src/config/signing.rs](../../../../../packages/functions/auth/src/config/signing.rs).

## What

Selection of local-es256 or kms-es256 and required key references.

## Why

Key custody can change without changing the first-party ES256 token format.

## How

Both modes require a key ID. Local mode requires a private-key secret reference; KMS mode requires a KMS key ID. Irrelevant mode-specific fields are omitted from the resulting configuration.
