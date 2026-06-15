# 09_google_oidc_callback_and_identity

## Goal

Complete the Google OIDC login loop that slice 08 started.

At the end of this slice, a browser returning from Google to `GET /google/callback` should cause the auth Lambda to consume typed provider state, consume the matching typed authorize session, exchange the Google code, validate the Google ID token, map Google `issuer + sub` to an internal subject, create a typed internal OAuth authorization code, and redirect back to the configured OAuth client callback with `code` and the original client `state`.

This slice intentionally stops at internal authorization-code issuance. The existing `/token` endpoint remains responsible for exchanging that internal code for access tokens, ID tokens, and optional refresh tokens.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/providers/google.md`
- `design/auth/providers/google.md`
- `design/auth/store/provider-states.md`
- `design/auth/store/authorize-sessions.md`
- `design/auth/store/authorization-codes.md`
- `design/auth/store/identities.md`
- `design/auth/core/identities.md`
- `design/auth/core/tokens.md`
- `design/auth/store/keys.md`
- `design/auth/observability/audit.md`
- `design/auth/testing.md`
- `design/scope.md`

The important design constraint is that Google remains first-class OIDC support, not a generic OAuth2/OIDC provider registry. This slice must not depend on legacy `ProviderConfig::Oidc`, the legacy generic provider router, hosted UI, or email-based account linking.

## Why This Slice Next

Slice 08 created the safe start boundary:

```text
/authorize provider=google -> typed authorize session -> typed provider state -> Google redirect
```

The next useful boundary is:

```text
Google callback code+state -> verified Google identity -> internal authorization code
```

That makes Google usable with the already implemented target `/token`, `/userinfo`, refresh rotation, and logout paths without changing token exchange in the same slice.

## In Scope

### Google Callback Route

Add the API-only callback route:

```text
GET /google/callback?code=<google-code>&state=<raw-provider-state>
```

Required behavior:

1. Require Google to be enabled.
2. Require `state`.
3. HMAC the raw provider state with `LookupFamily::ProviderState`.
4. Consume provider state with `take_provider_state`.
5. Require the provider-state record to be for `google`.
6. Consume the authorize session referenced by `session_lookup_digest`.
7. Require the authorize session to have `selected_provider=google`.
8. Require `code` unless Google returned an error.
9. Exchange the Google authorization code using the stored provider PKCE verifier.
10. Validate the returned Google ID token, including provider nonce.
11. Resolve or create the internal Google identity and subject.
12. Require the internal account to be active.
13. Create a typed internal authorization code.
14. Redirect to the original registered client `redirect_uri` with `code` and original client `state`.

The route must not render HTML and must not return tokens.

### Google Error Callback Handling

Support safe handling for Google error responses:

```text
GET /google/callback?error=access_denied&state=<raw-provider-state>
```

If the provider state and authorize session can be consumed, redirect back to the registered OAuth client redirect URI with:

```text
error=access_denied
state=<original-client-state>
```

If the provider state or authorize session cannot be trusted, return a local OAuth error instead of redirecting to an untrusted location.

### Google Code Exchange

Add Google-specific token exchange in the target provider module.

Target code:

```text
packages/functions/auth/src/providers/google.rs
```

Required request shape:

```text
POST https://oauth2.googleapis.com/token
grant_type=authorization_code
code=<google-code>
redirect_uri=<issuer_url>/google/callback
client_id=<AUTH_GOOGLE_CLIENT_ID>
client_secret=<AUTH_GOOGLE_CLIENT_SECRET>
code_verifier=<provider-pkce-verifier>
```

Required response behavior:

- Require an `id_token`.
- Ignore Google refresh tokens for v1.
- Do not store Google access tokens, refresh tokens, ID tokens, authorization codes, or code verifiers in DynamoDB.
- Do not include Google tokens or client secrets in logs, errors, audit events, or test snapshots.

### Google ID Token Validation

Validate Google ID tokens in the target Google provider module.

Required validation:

- JWT header uses a supported Google signing algorithm. For v1, accept `RS256`.
- JWT signature validates against Google JWKS.
- `iss` is exactly `https://accounts.google.com`.
- `aud` contains configured Google client ID.
- `exp` is valid.
- `iat` is not implausibly far in the future.
- `nonce` matches the provider nonce stored in provider state.
- `sub` is present and non-empty.

The provider nonce is the Google nonce from `ProviderStateRecord`. It is distinct from the first-party OIDC client nonce stored on the authorize session. The first-party client nonce must be copied to the internal authorization code for later ID-token issuance by `/token`.

### Google Provider Client Boundary

Introduce a small testable boundary for Google network behavior.

Suggested code shape:

```text
GoogleOidcClient trait
ReqwestGoogleOidcClient production implementation
FakeGoogleOidcClient test implementation
```

The route should call the boundary rather than directly constructing network calls inline. This keeps route tests deterministic and avoids live Google requests in the test suite.

The production implementation should reuse a `reqwest::Client`. JWKS caching may be a simple bounded in-memory cache if it is cheap to add; otherwise per-callback JWKS fetch is acceptable for this slice, with a follow-up optimization note. Correct validation is more important than cache sophistication in this slice.

### Google Identity Mapping

Resolve Google identities by issuer plus provider subject, never by email.

Identity digest input:

```text
raw_identity = google_issuer + "\n" + google_sub
identity_digest = HMAC-SHA256(storage_lookup_secret, LookupFamily::GoogleIdentity, raw_identity)
```

Required behavior:

- If no Google identity exists, create a new internal account and Google identity transactionally.
- If an active Google identity exists, reuse its persisted subject.
- If the mapped account is not active, reject login.
- If a deleted Google identity exists, apply the configured deleted-identity reuse policy.
- If reuse is allowed, create a new account and new active identity mapping with a new subject.
- Do not auto-link to a password identity with the same email.
- Do not auto-link to another Google identity by email.

Minimal Google identity properties may include:

```json
{
  "provider": "google",
  "issuer": "https://accounts.google.com",
  "email": "optional",
  "email_verified": true,
  "name": "optional",
  "picture": "optional"
}
```

The raw Google `sub` should not be stored in DynamoDB keys. Prefer not storing raw `sub` in the value either because the digest is sufficient for lookup.

### Identity Store Alignment

Align `IdentityRecord` with `design/auth/store/identities.md` by storing `last_seen_at` for active identities.

Required operations:

```text
resolve_or_create_google_identity
touch_identity_last_seen
```

The implementation may keep the existing generic account/identity transaction helpers if they are still clear, but provider route code should call a purpose-specific Google identity operation rather than manipulating raw account and identity records directly.

### Internal Authorization Code Issuance

After Google proof and active account checks, create a typed internal authorization code.

Record shape:

```json
{
  "client_id": "...",
  "redirect_uri": "...",
  "subject": "user_...",
  "subject_type": "user",
  "properties": {
    "provider": "google",
    "email": "optional",
    "email_verified": true
  },
  "code_challenge": "...",
  "code_challenge_method": "S256",
  "scope": "openid email",
  "oidc_nonce": "optional first-party client nonce",
  "created_at": "...",
  "expires_at": "..."
}
```

Rules:

- Raw internal authorization codes are returned only through the registered client redirect URI.
- DynamoDB keys use HMAC lookup digests, never raw authorization codes.
- Authorization code TTL uses `AUTH_AUTH_CODE_TTL_SECONDS`.
- The original authorize session is consumed once.
- The original provider state is consumed once.
- The internal authorization code is compatible with the existing `/token` endpoint.

### Audit Events

Emit sanitized audit events where the existing audit layer makes this practical:

- `google_login_succeeded`
- `provider_login_failed`

Audit events must not include Google codes, provider state, provider nonce, PKCE verifier, Google access tokens, Google ID tokens, Google client secret, or raw Google `sub`.

If the current audit module shape makes full event wiring too broad, add a focused follow-up note and keep this slice from logging secrets.

## Out Of Scope

- Apple login.
- Generic OAuth2 or generic OIDC provider support.
- Hosted login, hosted consent, account-selection UI, or provider-selection UI.
- Account linking between password and Google identities.
- Google refresh-token storage.
- Changes to `/token`, refresh rotation, `/userinfo`, or `/oauth/revoke` beyond compatibility tests.
- Token introspection.
- Opaque access tokens.
- IAM-protected admin lifecycle routes.
- Production JWKS cache tuning beyond a simple implementation.
- Live Google or live AWS validation.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree.

Target modules:

```text
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/providers/google.rs
packages/functions/auth/src/config.rs
packages/functions/auth/src/main.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/authorization_codes.rs
packages/functions/auth/src/store/provider_states.rs
packages/functions/auth/src/store/authorize_sessions.rs
packages/functions/auth/src/core/identities.rs
packages/functions/auth/src/crypto/hmac_lookup.rs
packages/functions/auth/tests/google_oidc_callback_slice.rs
packages/functions/auth/tests/google_oidc_start_slice.rs
packages/functions/auth/tests/token_exchange_slice.rs
```

Avoid using these legacy modules for the target Google callback:

```text
packages/functions/auth/src/provider/google.rs
packages/functions/auth/src/provider/oidc.rs
packages/functions/auth/src/provider/oauth2.rs
```

They may remain compiled until the legacy-removal slice, but the target route must not depend on them.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing domain tests for Google identity digest derivation from `issuer + "\n" + sub`.
2. Add failing ID-token validation tests for valid token, wrong issuer, wrong audience, wrong nonce, expired token, future `iat`, and missing `sub`.
3. Implement Google ID-token claim structs, JWKS parsing, and validation in `providers/google.rs`.
4. Add failing Google code-exchange tests using a fake HTTP boundary or fake `GoogleOidcClient`.
5. Implement the Google provider client boundary and production `ReqwestGoogleOidcClient`.
6. Add failing identity store tests for new Google identity, returning Google identity, deleted identity blocked by policy, deleted identity reused with a new subject when policy allows, and no email auto-linking to password identities.
7. Implement purpose-specific Google identity resolution in the store/core layer.
8. Add failing callback route tests for missing state, invalid state, Google error callback, successful callback redirect, wrong provider state provider, wrong authorize-session provider, failed ID-token validation, and one-time state/session consumption.
9. Implement `GET /google/callback` and mount it before legacy generic provider routes.
10. Add route/storage tests proving raw Google provider state, raw Google code, raw Google ID token, raw Google access token, raw Google client secret, and raw internal authorization code do not appear in DynamoDB keys.
11. Add compatibility test proving the internal code issued by Google callback can be exchanged through existing `/token`.
12. Update docs if implementation decisions refine the Google provider boundary or identity store shape.
13. Run full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Google Domain Tests

- Google identity digest is based on issuer plus `sub`, not email.
- Same Google `sub` with different issuer produces a different digest.
- Same email with different Google `sub` produces different identities.
- Valid Google ID token validates and returns issuer, subject, email, and email verification status.
- Wrong issuer fails.
- Wrong audience fails.
- Wrong nonce fails.
- Expired token fails.
- Token with future `iat` beyond allowed tolerance fails.
- Token with missing or empty `sub` fails.
- Unsupported JWT algorithm fails.
- JWKS without matching `kid` fails.

### Google Code Exchange Tests

- Token exchange sends `grant_type=authorization_code`.
- Token exchange sends configured Google client ID.
- Token exchange sends Google client secret only to the token endpoint boundary.
- Token exchange sends callback URI `<issuer_url>/google/callback`.
- Token exchange sends stored provider PKCE verifier.
- Token exchange requires `id_token` in the response.
- Token exchange errors do not include client secret, Google code, PKCE verifier, or provider tokens.

### Identity Store Tests

- First verified Google identity creates an account and identity mapping transactionally.
- Returning verified Google identity reuses the same subject.
- Returning verified Google identity updates `last_seen_at`.
- Deleted Google identity is blocked when reuse policy is `never`.
- Deleted Google identity inside retention is blocked when policy is `after_retention`.
- Reusable deleted Google identity creates a new subject, not the old subject.
- Password identity with the same email is not auto-linked to Google.
- Raw Google `sub` is not used in `pk` or `sk`.

### Callback Route Tests

- `GET /google/callback` is mounted.
- Missing `state` returns `400`.
- Unknown provider state returns `400`.
- Provider state is consumed once.
- Authorize session is consumed once after successful callback.
- Callback with a provider-state record for a non-Google provider returns `400`.
- Callback with an authorize session selected for `password` returns `400`.
- Google `error=access_denied` with valid state redirects back to the registered client with `error=access_denied` and original client `state`.
- Successful callback redirects to the registered client redirect URI.
- Successful callback redirect includes internal `code`.
- Successful callback redirect includes original client `state`.
- Successful callback creates a typed internal authorization code with HMAC lookup key.
- Authorization-code record carries the first-party client OIDC nonce, not the Google provider nonce.
- Failed ID-token validation does not create an internal authorization code.
- Callback response and storage keys do not expose Google code, provider state, ID token, access token, client secret, PKCE verifier, or raw internal authorization code.

### Token Compatibility Tests

- Internal authorization code produced by Google callback can be exchanged through existing `/token`.
- Token response includes ID token when scope includes `openid`.
- Token response includes email claims only when granted scope includes `email`.
- Token response does not include Google provider tokens.

## Acceptance Criteria

- Google callback is API-only and mounted at `GET /google/callback`.
- Google provider state is HMAC-keyed, short-lived, and single-use.
- Google callback consumes the matching authorize session exactly once.
- Google code exchange uses the stored provider PKCE verifier.
- Google ID token signature, issuer, audience, expiry, `iat`, nonce, and subject are validated.
- Google identity lookup is based on issuer plus `sub`, not email.
- Matching password and Google emails do not auto-link accounts.
- Deleted identity reuse follows configured account lifecycle policy.
- Successful Google login creates a typed internal authorization code compatible with `/token`.
- First-party OIDC client nonce is preserved into the internal authorization code.
- Google provider nonce is not copied into first-party ID tokens.
- Raw Google codes, provider states, access tokens, ID tokens, client secrets, provider PKCE verifiers, raw Google `sub`, and raw internal authorization codes are not stored in DynamoDB keys, logs, or errors.
- Existing password, token exchange, refresh, logout, and reset tests continue to pass.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

Manual protocol smoke test after implementation, using a real Google OAuth client only after local tests pass:

```text
GET /authorize?response_type=code&client_id=web&redirect_uri=<registered>&state=abc&scope=openid%20email&provider=google&code_challenge=<challenge>&code_challenge_method=S256
GET /google/authorize?session=<raw-session-from-authorize-redirect>
GET /google/callback?code=<google-code>&state=<google-provider-state>
POST /token grant_type=authorization_code code=<internal-code> redirect_uri=<registered> code_verifier=<client-verifier>
```

Expected result:

- `/google/callback` redirects to the registered client callback with an internal authorization code.
- `/token` exchanges that internal code for runtime-signed tokens.
- DynamoDB contains no raw Google provider state, Google code, Google tokens, Google client secret, provider PKCE verifier, or raw internal authorization code in `pk` or `sk`.

Live Google validation should not use production credentials until the AWS dev stage has stage-specific secrets and redirect URIs configured.

## Next Slice

After this slice, implement `10_apple_oidc_login` only if Google callback and identity behavior is stable.

Apple should reuse the typed provider-state, authorize-session, authorization-code, identity, and no-auto-linking patterns from Google, while keeping Apple-specific client-secret and ID-token validation behavior separate.
