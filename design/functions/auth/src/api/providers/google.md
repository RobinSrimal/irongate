# Google HTTP Flow

Source: [packages/functions/auth/src/api/providers/google.rs](../../../../../../packages/functions/auth/src/api/providers/google.rs).

## What

Google authorize and GET callback orchestration.

## Why

Provider authentication must be bound to the client's original authorize session.

## How

Authorize checks configured provider/session and creates separate HMAC-keyed state with nonce and upstream PKCE. Callback consumes state/session, exchanges code, validates the signed ID token and resolves a persisted identity before issuing a first-party code. Missing JWKS kid triggers one forced refresh/revalidation.

Code issuance captures the active account generation and conditions the write on that account
snapshot. A confirmed stale-account condition rejects the grant; operational store failures remain
server errors. Provider validation and identity resolution do not depend on password credentials.
