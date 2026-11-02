# Token Endpoint

Source: [packages/functions/auth/src/oauth/token.rs](../../../../../packages/functions/auth/src/oauth/token.rs).

## What

Client authentication and authorization-code/refresh grants.

## Why

Token issuance joins validated protocol inputs, active-account checks, typed one-use storage and the configured signer.

## How

Request rate limiting precedes authentication; unknown client IDs share a bounded identifier. Code client/redirect/PKCE checks precede conditional consumption. Tokens are signed after consumption; offline_access and grant permission gate refresh issuance. Refresh rotation precedes signing and returns no ID token. Storage and signing failures are not an all-or-nothing external response transaction.
