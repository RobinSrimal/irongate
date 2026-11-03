# Google HTTP Flow

Source: [packages/functions/auth/src/api/providers/google.rs](../../../../../../packages/functions/auth/src/api/providers/google.rs).

## What

Google authorize and GET callback orchestration.

## Why

Provider authentication must be bound to the client's original authorize session.

## How

Authorize checks configured provider/session and creates separate HMAC-keyed state with nonce and
upstream PKCE. Callback consumes state/session, exchanges code and validates the signed ID token
before resolving a persisted identity and issuing a first-party code. MissingKey, which includes
absent/blank/unmatched IDs, permits one forced refresh and full revalidation. A failed refresh or
second rejection stops the callback; other signature/key/claim failures do not force refresh.
Invalid callbacks cannot resolve/update identities or create accounts/codes, but their state/session
has already been consumed. Fake-client tests establish this orchestration, not HTTP cache freshness.

Code issuance captures the active account generation and conditions the write on that account
snapshot. A confirmed stale-account condition rejects the grant; operational store failures remain
server errors. Provider validation and identity resolution do not depend on password credentials.
