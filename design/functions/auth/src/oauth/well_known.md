# Discovery And JWKS

Source: [packages/functions/auth/src/oauth/well_known.rs](../../../../../packages/functions/auth/src/oauth/well_known.rs).

## What

OAuth/OIDC discovery metadata and the runtime signer's public keys.

## Why

Clients and resource servers need one advertised issuer and key source.

## How

Metadata exposes code/refresh grants, mounted endpoints, ES256, S256 and supported scopes/client authentication methods. JWKS delegates to the configured signer; no table private-key lookup occurs. Only the current configured signing key is returned.
