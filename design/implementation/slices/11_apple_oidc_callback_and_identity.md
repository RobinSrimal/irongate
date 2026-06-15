# 11_apple_oidc_callback_and_identity

## Goal

Complete the Sign in with Apple login loop that slice 10 started.

At the end of this slice, a browser returning from Apple to `POST /apple/callback` should cause the auth Lambda to consume typed provider state, consume the matching typed authorize session, generate an Apple client-secret JWT, exchange the Apple authorization code, validate the Apple ID token, map Apple `issuer + sub` to an internal subject, create a typed internal OAuth authorization code, and redirect back to the configured OAuth client callback with `code` and the original client `state`.

This slice intentionally stops at internal authorization-code issuance. The existing `/token` endpoint remains responsible for exchanging that internal code for access tokens, ID tokens, and optional refresh tokens.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/providers/apple.md`
- `design/auth/providers/apple.md`
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

The important design constraint is that Apple remains first-class OIDC support, not a generic OAuth2/OIDC provider registry. This slice must not depend on legacy `ProviderConfig::Oidc`, the legacy generic provider router, hosted UI, or email-based account linking.

## Why This Slice Next

Slice 10 created the safe Apple start boundary:

```text
/authorize provider=apple -> typed authorize session -> typed provider state -> Apple redirect
```

The next useful boundary is:

```text
Apple form-post callback code+state -> verified Apple identity -> internal authorization code
```

That makes Apple usable with the already implemented target `/token`, `/userinfo`, refresh rotation, and logout paths without changing token exchange in the same slice.

## In Scope

### Apple Callback Route

Add the API-only callback route:

```text
POST /apple/callback
Content-Type: application/x-www-form-urlencoded

code=<apple-code>&state=<raw-provider-state>
```

Apple was started with `response_mode=form_post`, so the target callback is a form POST, not a query-string GET.

Required behavior:

1. Require Apple to be enabled.
2. Require `state`.
3. HMAC the raw provider state with `LookupFamily::ProviderState`.
4. Consume provider state with `take_provider_state`.
5. Require the provider-state record to be for `apple`.
6. Consume the authorize session referenced by `session_lookup_digest`.
7. Require the authorize session to have `selected_provider=apple`.
8. Require `code` unless Apple returned an error.
9. Generate an Apple client-secret JWT from the configured Apple key.
10. Exchange the Apple authorization code using the generated client secret and stored provider PKCE verifier.
11. Validate the returned Apple ID token, including provider nonce.
12. Resolve or create the internal Apple identity and subject.
13. Require the internal account to be active.
14. Create a typed internal authorization code.
15. Redirect to the original registered client `redirect_uri` with `code` and original client `state`.

The route must not render HTML and must not return tokens.

### Apple Error Callback Handling

Support safe handling for Apple error responses:

```text
POST /apple/callback
Content-Type: application/x-www-form-urlencoded

error=access_denied&state=<raw-provider-state>
```

If the provider state and authorize session can be consumed, redirect back to the registered OAuth client redirect URI with:

```text
error=access_denied
state=<original-client-state>
```

If the provider state or authorize session cannot be trusted, return a local OAuth error instead of redirecting to an untrusted location.

### Apple Code Exchange

Add Apple-specific token exchange in the target provider module.

Target code:

```text
packages/functions/auth/src/providers/apple.rs
```

Required request shape:

```text
POST https://appleid.apple.com/auth/token
grant_type=authorization_code
code=<apple-code>
redirect_uri=<issuer_url>/apple/callback
client_id=<AUTH_APPLE_CLIENT_ID>
client_secret=<generated-apple-client-secret-jwt>
code_verifier=<provider-pkce-verifier>
```

Required response behavior:

- Require an `id_token`.
- Ignore Apple refresh tokens for v1.
- Do not store Apple access tokens, refresh tokens, ID tokens, authorization codes, generated client secrets, or code verifiers in DynamoDB.
- Do not include Apple tokens, generated client secrets, private keys, codes, or code verifiers in logs, errors, audit events, or test snapshots.

### Apple Provider Client Boundary

Introduce a small testable boundary for Apple network behavior.

Suggested code shape:

```text
AppleOidcClient trait
ReqwestAppleOidcClient production implementation
FakeAppleOidcClient test implementation
```

The route should call the boundary rather than directly constructing network calls inline. This keeps route tests deterministic and avoids live Apple requests in the test suite.

The production implementation should reuse a `reqwest::Client`. JWKS caching may be a simple bounded in-memory cache if it is cheap to add; otherwise per-callback JWKS fetch is acceptable for this slice, with a follow-up optimization note. Correct validation is more important than cache sophistication in this slice.

### Apple ID Token Validation

Validate Apple ID tokens in the target Apple provider module.

Required validation:

- JWT header uses a supported Apple signing algorithm. For v1, accept `RS256`.
- JWT signature validates against Apple JWKS.
- `iss` is exactly `https://appleid.apple.com`.
- `aud` contains configured Apple client ID.
- `exp` is valid.
- `iat` is not implausibly far in the future.
- `nonce` matches the provider nonce stored in provider state.
- `sub` is present and non-empty.

The provider nonce is the Apple nonce from `ProviderStateRecord`. It is distinct from the first-party OIDC client nonce stored on the authorize session. The first-party client nonce must be copied to the internal authorization code for later ID-token issuance by `/token`.

Apple profile claims are optional:

- `email` may be absent.
- `email_verified` may be represented as a boolean or string by provider fixtures and should be normalized carefully.
- `is_private_email` may be absent and must not change identity matching.
- The optional `user` form field may be accepted for future profile enrichment, but this slice must not require it and must not treat it as identity proof.

### Apple Identity Mapping

Resolve Apple identities by issuer plus provider subject, never by email.

Identity digest input:

```text
raw_identity = apple_issuer + "\n" + apple_sub
identity_digest = HMAC-SHA256(storage_lookup_secret, LookupFamily::AppleIdentity, raw_identity)
```

Required behavior:

- If no Apple identity exists, create a new internal account and Apple identity transactionally.
- If an active Apple identity exists, reuse its persisted subject.
- If the mapped account is not active, reject login.
- If a deleted Apple identity exists, apply the configured deleted-identity reuse policy.
- If reuse is allowed, create a new account and new active identity mapping with a new subject.
- Do not auto-link to a password identity with the same email.
- Do not auto-link to a Google identity with the same email.
- Do not auto-link to another Apple identity by email.

Minimal Apple identity properties may include:

```json
{
  "provider": "apple",
  "issuer": "https://appleid.apple.com",
  "email": "optional",
  "email_verified": true,
  "is_private_email": "optional"
}
```

The raw Apple `sub` should not be stored in DynamoDB keys. Prefer not storing raw `sub` in the value either because the digest is sufficient for lookup.

### Identity Store Alignment

Reuse the existing identity store model already used by password and Google identities.

Required operation:

```text
resolve_or_create_apple_identity
```

The implementation may factor the existing Google identity resolution into a provider-generic helper if that reduces duplication without broadening the public route API. Provider route code should still call a purpose-specific Apple operation rather than manipulating raw account and identity records directly.

### Internal Authorization Code Issuance

After Apple proof and active account checks, create a typed internal authorization code.

Record shape:

```json
{
  "client_id": "...",
  "redirect_uri": "...",
  "subject": "user_...",
  "subject_type": "user",
  "properties": {
    "provider": "apple",
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

- `apple_login_succeeded`
- `provider_login_failed`

Audit events must not include Apple codes, provider state, provider nonce, PKCE verifier, Apple access tokens, Apple ID tokens, generated Apple client secrets, Apple private keys, or raw Apple `sub`.

If the current audit module shape makes full event wiring too broad, add a focused follow-up note and keep this slice from logging secrets.

## Out Of Scope

- Google login changes.
- Password login, registration, verification, or reset changes.
- Generic OAuth2 or generic OIDC provider support.
- Hosted login, hosted consent, account-selection UI, or provider-selection UI.
- Account linking between password, Google, and Apple identities.
- Apple refresh-token storage.
- Changes to `/token`, refresh rotation, `/userinfo`, or `/oauth/revoke` beyond compatibility tests.
- Token introspection.
- Opaque access tokens.
- IAM-protected admin lifecycle routes.
- Production JWKS cache tuning beyond a simple implementation.
- Live Apple developer account validation.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree.

Target modules:

```text
packages/functions/auth/src/api/providers/apple.rs
packages/functions/auth/src/providers/apple.rs
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
packages/functions/auth/tests/apple_oidc_callback_slice.rs
packages/functions/auth/tests/apple_oidc_start_slice.rs
packages/functions/auth/tests/token_exchange_slice.rs
```

Avoid using these legacy modules for the target Apple callback:

```text
packages/functions/auth/src/provider/apple.rs
packages/functions/auth/src/provider/oidc.rs
packages/functions/auth/src/provider/oauth2.rs
```

They may remain compiled until the legacy-removal slice, but the target route must not depend on them.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing domain tests for Apple identity digest derivation from `issuer + "\n" + sub`.
2. Add failing ID-token validation tests for valid token, wrong issuer, wrong audience, wrong nonce, expired token, future `iat`, missing `sub`, unsupported algorithm, and missing JWKS key.
3. Implement Apple ID-token claim structs, JWKS parsing, and validation in `providers/apple.rs`.
4. Add failing Apple code-exchange tests using a fake HTTP boundary or fake `AppleOidcClient`.
5. Implement the Apple provider client boundary and production `ReqwestAppleOidcClient`.
6. Add failing identity store tests for new Apple identity, returning Apple identity, deleted identity blocked by policy, deleted identity reused with a new subject when policy allows, and no email auto-linking to password or Google identities.
7. Implement purpose-specific Apple identity resolution in the store/core layer.
8. Add failing callback route tests for missing state, invalid state, Apple error callback, successful form-post callback redirect, wrong provider state provider, wrong authorize-session provider, failed ID-token validation, and one-time state/session consumption.
9. Implement `POST /apple/callback` and mount it before legacy generic provider routes.
10. Add route/storage tests proving raw Apple provider state, raw Apple code, raw Apple ID token, raw Apple access token, generated client secret, Apple private key, and raw internal authorization code do not appear in DynamoDB keys.
11. Add compatibility test proving the internal code issued by Apple callback can be exchanged through existing `/token`.
12. Update docs if implementation decisions refine the Apple provider boundary or identity store shape.
13. Run full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Apple Domain Tests

- Apple identity digest is based on issuer plus `sub`, not email.
- Same Apple `sub` with different issuer produces a different digest.
- Same email with different Apple `sub` produces different identities.
- Valid Apple ID token validates and returns issuer, subject, optional email, optional email verification status, and optional private-email status.
- Wrong issuer fails.
- Wrong audience fails.
- Wrong nonce fails.
- Expired token fails.
- Token with future `iat` beyond allowed tolerance fails.
- Token with missing or empty `sub` fails.
- Unsupported JWT algorithm fails.
- JWKS without matching `kid` fails.

### Apple Code Exchange Tests

- Token exchange sends `grant_type=authorization_code`.
- Token exchange sends configured Apple client ID.
- Token exchange sends generated Apple client-secret JWT only to the token endpoint boundary.
- Token exchange sends callback URI `<issuer_url>/apple/callback`.
- Token exchange sends stored provider PKCE verifier.
- Token exchange requires `id_token` in the response.
- Token exchange errors do not include private key material, generated client secret, Apple code, PKCE verifier, or provider tokens.

### Identity Store Tests

- First verified Apple identity creates an account and identity mapping transactionally.
- Returning verified Apple identity reuses the same subject.
- Returning verified Apple identity updates `last_seen_at`.
- Deleted Apple identity is blocked when reuse policy is `never`.
- Deleted Apple identity inside retention is blocked when policy is `after_retention`.
- Reusable deleted Apple identity creates a new subject, not the old subject.
- Password identity with the same email is not auto-linked to Apple.
- Google identity with the same email is not auto-linked to Apple.
- Raw Apple `sub` is not used in `pk` or `sk`.

### Callback Route Tests

- `POST /apple/callback` is mounted.
- `GET /apple/callback` is not the target callback path.
- Missing `state` returns `400`.
- Unknown provider state returns `400`.
- Provider state is consumed once.
- Authorize session is consumed once after successful callback.
- Callback with a provider-state record for a non-Apple provider returns `400`.
- Callback with an authorize session for a non-Apple provider returns `400`.
- Apple error callback redirects to the registered client redirect URI only after trusted state/session are consumed.
- Successful callback redirects to the registered client redirect URI with `code` and original client `state`.
- Successful callback creates an authorization code record with `provider=apple`.
- Successful callback preserves first-party OIDC `nonce` for later ID-token issuance.
- Successful callback response does not contain access tokens, refresh tokens, ID tokens, Apple client secrets, or Apple private key material.
- Failed callback responses do not leak Apple code, provider state, provider nonce, PKCE verifier, generated client secret, private key material, or ID token.

### Compatibility Tests

- The internal authorization code issued by Apple callback can be exchanged through `/token`.
- `/token` returns runtime-signed access tokens and OIDC ID tokens for Apple-backed subjects.
- `/userinfo` works for an Apple-backed access token and returns only claims allowed by scope.

## Acceptance Criteria

- Apple login completes from form-post callback to internal authorization-code redirect.
- Apple callback is API-only and renders no HTML.
- Apple code exchange uses a generated client-secret JWT and the stored provider PKCE verifier.
- Apple ID tokens are validated for signature, issuer, audience, expiry, issued-at tolerance, nonce, and subject.
- Apple identity uses `issuer + sub`, not email.
- Apple sign-in does not auto-link to password or Google identities by matching email.
- Only active Apple-backed accounts can receive authorization codes.
- Deleted Apple identity reuse follows configured deleted-identity reuse policy.
- Raw Apple provider state, Apple authorization code, Apple ID token, generated client secret, private key material, provider nonce, PKCE verifier, and internal authorization code are not stored in DynamoDB keys, logs, errors, or audit events.
- Internal authorization codes issued by Apple callback are compatible with existing `/token`, `/userinfo`, refresh, and logout paths.
- Google and password flows continue to pass unchanged.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test apple_oidc_callback_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

Manual protocol smoke test should wait until an Apple developer client is available:

```text
GET /authorize?...&provider=apple
POST /apple/callback
POST /token
GET /userinfo
```

Expected result:

- `/authorize` creates a short-lived typed authorize session.
- `/apple/authorize` redirects to Apple with form-post callback mode.
- `/apple/callback` consumes provider state and authorize session once.
- Client callback receives an internal code and original state.
- `/token` exchanges that internal code for runtime-signed tokens.
- DynamoDB contains no raw provider state, raw Apple code, raw Apple ID token, generated client secret, Apple private key material, or raw internal authorization code in `pk` or `sk`.

Live Apple and AWS validation are not required for this slice, but should be part of the later AWS hardening and runtime validation slice.

## Next Slice

After this slice, implement `12_iam_admin_account_lifecycle`.

That slice should add IAM-protected operator lifecycle routes for disabling users, deleting users, and revoking subject sessions without reintroducing public bootstrap routes, custom admin API keys, or dashboard-only assumptions.
