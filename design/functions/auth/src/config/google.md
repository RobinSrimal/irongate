# Google Configuration

Source: [packages/functions/auth/src/config/google.rs](../../../../../packages/functions/auth/src/config/google.rs).

## What

Fixed Google OIDC endpoints and optional web client credentials.

## Why

The provider is enabled only with a complete credential pair.

## How

Absent inputs disable Google; partial inputs fail. Client-secret Debug output is redacted; issuer, token endpoint, authorization endpoint and JWKS URI are fixed in the module.
