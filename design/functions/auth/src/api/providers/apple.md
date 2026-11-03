# Apple HTTP Flow

Source: [packages/functions/auth/src/api/providers/apple.rs](../../../../../../packages/functions/auth/src/api/providers/apple.rs).

## What

Apple authorize and form-post callback orchestration.

## Why

Apple's response mode and client-secret JWT need provider-specific handling without changing OAuth client contracts.

## How

Creates bound state/nonce/upstream PKCE, exchanges the returned code using an Apple client-secret JWT and validates the ID token. One missing-key refresh is allowed. Identity resolution and first-party code issuance use AuthStore; unsigned callback profile data is not an identity proof.

Code issuance captures the active account generation and conditions the write on that account
snapshot. A confirmed stale-account condition rejects the grant; operational store failures remain
server errors. Provider validation and identity resolution do not depend on password credentials.
