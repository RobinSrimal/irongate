# Google Start Tests

Source: [packages/functions/auth/tests/oidc_google_start.rs](../../../../packages/functions/auth/tests/oidc_google_start.rs).

## What

Provider-state expiry/consumption and Google start redirects.

## Why

Upstream nonce/PKCE and internal session binding must survive the handoff.

## How

Tests construct config and typed sessions, inspect HMAC-keyed state, and reject disabled providers, wrong-provider sessions and excessive requests. They use the memory backend and no provider network.

Rate-limit cases configure the secondary policy and require both source and provider/source rows,
queried from the bounded provider-start partition, with no raw IP or session in their keys.
Cross-provider/session source rotation lives in the
[admission suite](api_routes/admission.md).
