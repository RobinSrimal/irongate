# 08_google_oidc_start_and_provider_state

## Goal

Add the first half of Google OIDC login: runtime Google configuration, typed provider-state storage, and an API-only Google authorization redirect.

At the end of this slice, an application can start a normal OAuth authorize request with `provider=google`. The auth Lambda creates a typed authorize session, redirects to `/google/authorize?session=...`, creates a short-lived HMAC-keyed Google provider-state record, and redirects the browser to Google with `state`, `nonce`, and PKCE.

This slice intentionally stops before the Google callback. The next slice validates Google ID tokens, maps Google issuer+subject to an internal identity, and creates the internal OAuth authorization code.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/oauth/authorize.md`
- `design/auth/api/providers/google.md`
- `design/auth/providers/google.md`
- `design/auth/store/authorize-sessions.md`
- `design/auth/store/provider-states.md`
- `design/auth/store/keys.md`
- `design/auth/config/environment.md`
- `design/infra/secrets.md`
- `design/scope.md`

The important design constraint is that Google is first-class OIDC provider support, not a generic OAuth2 provider registry. This slice must not reintroduce hosted provider-selection UI, generic OAuth2 identity providers, raw provider state keys, or legacy provider state storage.

## Why This Slice Next

Slices 03-07 completed the first-party password account lifecycle. The next product capability in scope is Google OIDC.

Google callback validation and identity mapping are security-sensitive enough to keep separate. This slice builds the safe start boundary first:

```text
/authorize provider=google -> typed authorize session -> typed provider state -> Google redirect
```

That gives the next slice a clean, testable callback boundary:

```text
Google callback code+state -> consume typed provider state -> validate ID token -> issue internal authorization code
```

## In Scope

### Google Runtime Configuration

Add target Google configuration to runtime auth config.

Target code:

```text
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/config/google.rs
```

Recommended runtime variables:

```text
AUTH_GOOGLE_CLIENT_ID optional pair with AUTH_GOOGLE_CLIENT_SECRET
AUTH_GOOGLE_CLIENT_SECRET optional pair with AUTH_GOOGLE_CLIENT_ID
```

Rules:

- If both Google variables are absent, Google is disabled and password-only deployments still start.
- If exactly one Google variable is present, startup fails clearly.
- If both are present, Google is enabled.
- Google authorization, token, issuer, and JWKS URLs are fixed constants in code.
- The Google client secret is not logged, not stored in DynamoDB, and not exposed through debug output.
- Deployed stages should supply `AUTH_GOOGLE_CLIENT_SECRET` through SST secrets.

Fixed Google values:

```text
authorization_url = https://accounts.google.com/o/oauth2/v2/auth
token_url = https://oauth2.googleapis.com/token
issuer = https://accounts.google.com
jwks_uri = https://www.googleapis.com/oauth2/v3/certs
scopes = openid email profile
```

### Typed Provider State Store

Add typed provider-state storage.

Target code:

```text
packages/functions/auth/src/store/provider_states.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/mod.rs
```

Required store operations:

```text
create_provider_state
take_provider_state
```

Record shape:

```json
{
  "session_lookup_digest": "...",
  "provider": "google",
  "pkce_verifier": "...",
  "nonce": "...",
  "created_at": "...",
  "expires_at": "..."
}
```

Rules:

- Raw provider state is generated with high entropy.
- DynamoDB keys use HMAC lookup digests, never raw provider state.
- Records carry `expires_at`.
- DynamoDB TTL uses the same expiry value.
- Provider states are single-use.
- Expired provider states are rejected even if DynamoDB TTL has not deleted them.
- The record stores the authorize-session lookup digest, not the raw authorize-session key.
- Routes and providers must not use generic `set` or `get` for provider-state records.

### Authorize Endpoint Provider Selection

Update `/authorize` to accept `provider=google`.

Target code:

```text
packages/functions/auth/src/oauth/authorize.rs
```

Required behavior:

- Continue requiring an explicit provider.
- Continue rejecting unsupported providers.
- Accept `provider=password` with existing behavior.
- Accept `provider=google` only when Google is enabled in runtime config.
- Store the typed authorize session exactly as today, with `selected_provider=google`.
- Redirect to:

```text
/google/authorize?session=<raw-authorize-session-key>
```

The endpoint must not redirect directly to Google. Google-specific state, nonce, and PKCE are owned by the Google provider-start route.

### Google Provider Start Route

Add an API-only Google start route.

Target code:

```text
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/providers/google.rs
packages/functions/auth/src/routes.rs
```

Route:

```text
GET /google/authorize?session=<raw-authorize-session-key>
```

Required behavior:

1. Require Google to be enabled.
2. Require a `session` query parameter.
3. Compute the authorize-session lookup digest from the raw session key.
4. Verify the authorize session exists and has `selected_provider=google`.
5. Generate raw Google provider state.
6. Generate Google OIDC nonce.
7. Generate Google PKCE verifier and challenge.
8. Store provider state through `create_provider_state`.
9. Redirect to Google authorization URL.

The Google authorization URL must include:

```text
client_id
redirect_uri
response_type=code
scope=openid email profile
state=<raw-provider-state>
nonce=<raw-provider-nonce>
code_challenge=<S256 challenge>
code_challenge_method=S256
```

The callback redirect URI should be derived from the configured issuer URL:

```text
<issuer_url>/google/callback
```

For dev/test fallback only, if issuer URL is absent, use the same existing localhost fallback convention as other OAuth endpoints.

### Google Domain Helpers

Add small Google-specific helpers rather than a broad provider abstraction.

Target code:

```text
packages/functions/auth/src/providers/google.rs
```

Suggested helpers:

```text
GoogleConfig
GoogleAuthorizeInput
GoogleAuthorizeRedirect
build_google_authorization_url
start_google_authorize
```

Rules:

- URL construction is deterministic and unit-testable.
- Google scopes are fixed for v1.
- Provider nonce is separate from the first-party OIDC client nonce stored on the authorize session.
- The helper does not exchange Google codes or validate ID tokens in this slice.

## Out Of Scope

- `GET /google/callback`.
- Google code exchange.
- Google ID-token validation.
- Google JWKS fetching or caching.
- Google identity persistence.
- Authorization-code issuance after Google proof.
- Apple login.
- Generic OAuth2 or generic OIDC provider support.
- Hosted provider-selection UI.
- Account linking between password and Google identities.
- Refresh-token issuance changes.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree.

Target modules:

```text
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/api/providers/mod.rs
packages/functions/auth/src/providers/google.rs
packages/functions/auth/src/providers/mod.rs
packages/functions/auth/src/store/provider_states.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/config/google.rs
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/oauth/authorize.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/tests/google_oidc_start_slice.rs
packages/functions/auth/tests/startup_config_slice.rs
packages/functions/auth/tests/runtime_route_slice.rs
```

Legacy `packages/functions/auth/src/provider/google.rs`, `provider/oidc.rs`, and generic provider routes may remain compiled if they are not used by the target Google route. This slice must not depend on legacy `ProviderConfig::Oidc` or raw `provider:state` writes.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing config tests for Google disabled, enabled, and half-configured startup states.
2. Implement `config/google.rs` and add it to `RuntimeAuthConfig`.
3. Add failing store tests for provider-state HMAC keys, TTL, and single-use consumption.
4. Implement typed provider-state records and store operations.
5. Add failing Google URL-builder tests proving URL parameters, scopes, nonce, state, and PKCE challenge.
6. Implement Google URL-building helpers.
7. Add failing `/authorize provider=google` route tests.
8. Update `/authorize` to accept Google only when enabled and redirect to `/google/authorize`.
9. Add failing `GET /google/authorize` route tests.
10. Implement `GET /google/authorize`.
11. Add tests proving raw provider state and raw authorize session key do not appear in DynamoDB keys.
12. Run full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Config Tests

- Runtime config starts with Google disabled when both Google variables are absent.
- Runtime config enables Google when both Google variables are present.
- Runtime config fails when only `AUTH_GOOGLE_CLIENT_ID` is present.
- Runtime config fails when only `AUTH_GOOGLE_CLIENT_SECRET` is present.
- Runtime config debug output does not expose the Google client secret.

### Provider-State Store Tests

- `create_provider_state` stores by HMAC digest, not raw state.
- Provider-state record stores provider, session lookup digest, PKCE verifier, nonce, `created_at`, and `expires_at`.
- `take_provider_state` consumes once.
- Expired provider state is rejected.
- Raw provider state and raw authorize-session key do not appear in `pk` or `sk`.

### Google URL Tests

- Google authorization URL uses `https://accounts.google.com/o/oauth2/v2/auth`.
- URL includes configured Google client ID.
- URL includes callback URI `<issuer_url>/google/callback`.
- URL includes `scope=openid email profile`.
- URL includes raw state.
- URL includes provider nonce.
- URL includes S256 PKCE challenge and method.
- URL does not include Google client secret.

### Route Tests

- `/authorize` rejects `provider=google` when Google is disabled.
- `/authorize` accepts `provider=google` when Google is enabled.
- `/authorize` with Google creates a typed authorize session with `selected_provider=google`.
- `/authorize` with Google redirects to `/google/authorize?session=...`, not directly to Google.
- `GET /google/authorize` redirects to Google.
- `GET /google/authorize` creates one typed provider-state record.
- `GET /google/authorize` rejects missing session.
- `GET /google/authorize` rejects a session whose stored `selected_provider` is not `google`.
- Route responses and stored keys do not expose raw provider state, raw authorize-session key, Google client secret, access tokens, refresh tokens, or ID tokens.

## Acceptance Criteria

- Password-only deployments still work without Google credentials.
- Half-configured Google credentials fail startup.
- Google start uses fixed first-class Google OIDC configuration, not generic provider config.
- `/authorize provider=google` works only when Google is enabled.
- `/google/authorize` creates typed provider state with HMAC lookup key and TTL.
- Raw Google provider state is only sent to Google, not stored in DynamoDB keys.
- Raw authorize-session key is not stored in provider-state records or keys.
- Google authorization URL includes state, nonce, and PKCE.
- No Google callback, token exchange, ID-token validation, identity persistence, or internal authorization-code issuance is implemented in this slice.
- Existing password, token exchange, refresh, and reset tests continue to pass.
- The auth Lambda remains API-only and renders no hosted provider-selection UI.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

Manual protocol smoke test after implementation:

```text
GET /authorize?response_type=code&client_id=web&redirect_uri=<registered>&state=abc&scope=openid%20email&provider=google&code_challenge=<challenge>&code_challenge_method=S256
GET /google/authorize?session=<raw-session-from-authorize-redirect>
```

Expected result:

- `/authorize` creates a typed authorize session and redirects to `/google/authorize`.
- `/google/authorize` creates typed provider state and redirects to Google.
- DynamoDB contains no raw provider state or raw authorize-session key in `pk` or `sk`.

Live Google callback validation should wait for the next slice.

## Next Slice

After this slice, implement `09_google_oidc_callback_and_identity`.

That slice should:

- add `GET /google/callback`
- exchange the Google authorization code
- validate Google ID token issuer, audience, signature, expiry, and nonce
- map `https://accounts.google.com` plus Google `sub` to an internal subject
- create or reuse persisted Google identity records
- require active accounts
- create a typed internal OAuth authorization code
- redirect back to the registered OAuth client callback
