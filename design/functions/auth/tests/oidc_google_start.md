# Google Start Tests

Source: [packages/functions/auth/tests/oidc_google_start.rs](../../../../packages/functions/auth/tests/oidc_google_start.rs).

## What

Google start URL construction, session admission and redirects.

## Why

Upstream nonce/PKCE and internal session binding must survive the handoff.

## How

Tests construct config and typed sessions, inspect HMAC-keyed state, and reject disabled providers, wrong-provider sessions and excessive requests. They use the memory backend and no provider network.

Rate-limit cases configure the secondary policy and require both source and provider/source rows,
queried from the bounded provider-start partition, with no raw IP or session in their keys.
Cross-provider/session source rotation lives in the
[admission suite](api_routes/admission.md).

Shared [provider-state contracts](provider_states.md) cover single-use/expiry on both backends,
instead of duplicate store-only cases here. The [browser flow child](oidc_google_callback/browser_binding.md)
exercises actual starts, scoped cookies, session-capped TTL and matching callback completion.

Application-state fixtures explicitly install the shared [captured audit emitter](support/audit.md),
so tests do not write production stdout/stderr. Dedicated audit route suites assert emission itself.
