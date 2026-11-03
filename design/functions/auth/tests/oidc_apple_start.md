# Apple Start Tests

Source: [packages/functions/auth/tests/oidc_apple_start.rs](../../../../packages/functions/auth/tests/oidc_apple_start.rs).

## What

Client-secret JWTs, authorization URLs, provider enablement and one-use state creation.

## Why

Apple form_post and credential claims differ from the client-facing OAuth flow.

## How

Tests use local ES256 keys and configured fixtures to check state/nonce/PKCE, HMAC storage, rate limits and wrong-provider session rejection without external requests.

Rate-limit cases configure the secondary policy and require both source and provider/source rows,
queried from the bounded provider-start partition, with no raw IP or session in their keys.
Cross-provider/session source rotation lives in the
[admission suite](api_routes/admission.md).
