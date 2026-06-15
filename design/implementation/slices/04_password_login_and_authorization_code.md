# 04_password_login_and_authorization_code

## Goal

Implement first-party password login for already verified password accounts and issue OAuth authorization codes, without changing token exchange yet.

At the end of this slice, an application-owned login form can submit email, password, and an authorize session key to the auth Lambda. If the password identity is verified and the account is active, the auth Lambda consumes the authorize session, creates a short-lived authorization code, and redirects to the registered OAuth redirect URI with `code` and `state`.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/providers/password.md`
- `design/auth/providers/password.md`
- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/store/authorize-sessions.md`
- `design/auth/store/authorization-codes.md`
- `design/auth/store/password-users.md`
- `design/auth/core/account-lifecycle.md`
- `design/auth/core/scopes.md`
- `design/auth/core/tokens.md`
- `design/auth/store/keys.md`
- `design/scope.md`

The important design constraint is that the auth Lambda stays API-only. This slice must not add hosted login pages, hosted consent, provider-selection UI, account-selection UI, or frontend-framework assumptions.

## Why This Slice Next

Slice 03 created verified password-backed accounts but intentionally stopped before authentication. The next useful boundary is:

```text
verified password user + valid authorize session -> OAuth authorization code
```

This keeps the earlier security invariant intact:

```text
registration and email verification do not authenticate the user
```

Login becomes the first operation that can create an authorization code. Token issuance remains separate and comes in slice 05.

## In Scope

### Typed Authorize Session Store

Cut `/authorize` over to the target typed authorize-session storage model before password login consumes sessions.

Required store operations:

```text
create_authorize_session
take_authorize_session
```

Record shape:

```json
{
  "client_id": "...",
  "redirect_uri": "...",
  "state": "...",
  "scope": "openid email",
  "oidc_nonce": "optional",
  "code_challenge": "...",
  "code_challenge_method": "S256",
  "selected_provider": "password",
  "created_at": "...",
  "expires_at": "..."
}
```

Rules:

- Raw authorize session keys are generated with high entropy.
- DynamoDB keys use HMAC lookup digests, never raw session keys.
- Records carry `expires_at`.
- DynamoDB TTL uses the same expiry value.
- Consuming a session is single-use.
- Expired sessions are rejected even if DynamoDB TTL has not deleted them.
- OIDC client `nonce` is stored as `oidc_nonce`.

### Typed Authorization Code Store

Add typed authorization-code creation for the codes issued by password login.

Required store operations:

```text
create_authorization_code
```

`take_authorization_code` may be added if it is cheap, but token exchange is not cut over in this slice.

Record shape:

```json
{
  "client_id": "...",
  "redirect_uri": "...",
  "subject": "user_...",
  "subject_type": "user",
  "properties": {
    "email": "user@example.com",
    "email_verified": true,
    "provider": "password"
  },
  "code_challenge": "...",
  "code_challenge_method": "S256",
  "scope": "openid email",
  "oidc_nonce": "optional",
  "created_at": "...",
  "expires_at": "..."
}
```

Rules:

- Raw authorization codes are returned only through the redirect URI.
- DynamoDB keys use HMAC lookup digests, never raw authorization codes.
- Records carry `expires_at`.
- DynamoDB TTL uses the same expiry value.
- Codes are short-lived according to `AUTH_AUTH_CODE_TTL_SECONDS`.
- Token exchange support for this typed code format is slice 05.

### Authorize Endpoint Cutover

Update `/authorize` so it creates typed authorize sessions.

Required behavior:

- Validate the configured OAuth client.
- Require exact redirect URI.
- Require `response_type=code`.
- Require PKCE for public clients through the existing client registry.
- Reject unsupported `code_challenge_method` values.
- Store requested scope after validating it against the client allowed scopes.
- Store optional OIDC `nonce` when `openid` is requested.
- Store `provider=password` as selected provider when supplied.
- Do not render provider-selection UI.
- Do not issue tokens.

For this slice, the implementation may keep the existing redirect-to-provider handoff shape, but password login itself must be driven by `POST /password/login`. Any app-owned UI is outside this repository.

### Password Login Domain Operation

Add a password login domain operation that:

1. Normalizes the submitted email.
2. Computes the HMAC email digest.
3. Loads the password user record.
4. Verifies the Argon2id password hash.
5. Requires `verified=true`.
6. Requires the password user to have a persisted subject.
7. Requires the subject account to be active.
8. Consumes the authorize session by raw session key through HMAC lookup.
9. Creates a typed authorization code.
10. Returns the redirect URI to use for OAuth continuation.

Suggested input:

```json
{
  "session": "raw-authorize-session-key",
  "email": "user@example.com",
  "password": "correct horse battery staple"
}
```

Suggested domain output:

```json
{
  "status": "authorization_code_issued",
  "redirect_uri": "https://app.example.com/auth/callback?code=...&state=..."
}
```

The raw authorization code may appear in the redirect URI because that is the OAuth protocol handoff. It must not appear in DynamoDB keys, logs, or error messages.

### Password Login Route

Add an API-only route:

```text
POST /password/login
```

Route behavior:

- Accept `application/x-www-form-urlencoded` fields `session`, `email`, and `password`.
- On success, return `303 See Other` with `Location: <registered redirect_uri>?code=...&state=...`.
- Do not render HTML.
- Do not return access tokens, refresh tokens, or ID tokens.
- Do not expose the password hash, email digest, session digest, or authorization code digest.

Form encoding is preferred for this slice because it lets an application-owned login page submit directly to the auth Lambda and let the browser follow the OAuth redirect. JSON support can be added later only if a concrete app integration needs it.

### Account Active Check

Login must require an active account before creating an authorization code.

Current code has `active` and `deleted` account states. If the implementation adds `disabled` now, login must reject both `disabled` and `deleted`. If `disabled` remains deferred to the IAM admin lifecycle slice, login must still use a central `require_active_account` or equivalent so slice 07 can add disabled behavior without rewriting login.

### Password Flow Rate Limits

The password provider design requires login, registration, verification, and reset attempts to be rate-limited. Slice 03 added registration and verification before this was wired into the new API-only password routes, so this slice should close that gap for the password endpoints that exist so far.

Required behavior:

- Apply rate limits to `POST /password/register`.
- Apply rate limits to `POST /password/verify`.
- Apply rate limits to `POST /password/login`.
- Use the trusted source identity strategy already available to the router until API Gateway request-context source IP is cut over in the AWS hardening slice.
- Include a normalized email HMAC digest in the rate-limit identifier for registration and login when an email is present.
- Do not store raw email addresses, passwords, session keys, verification tokens, or authorization codes in rate-limit keys.
- Return `429 Too Many Requests` with the existing rate-limit response shape when the limit is exceeded.

## Out Of Scope

- `/token` cutover to typed authorization code consumption.
- Access-token, refresh-token, or ID-token issuance.
- Refresh-token storage rewrite.
- `/userinfo`.
- `/oauth/revoke`.
- Password reset.
- Google or Apple login.
- Hosted UI.
- Consent UI.
- Account-selection UI.
- IAM-protected account lifecycle admin routes.
- Generic OIDC provider support.

## Expected Code Shape

Current repo paths should be followed rather than inventing a parallel tree.

Target modules:

```text
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/providers/password.rs
packages/functions/auth/src/oauth/authorize.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/store/authorize_sessions.rs
packages/functions/auth/src/store/authorization_codes.rs
packages/functions/auth/src/store/password_users.rs
packages/functions/auth/src/store/rate_limits.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/core/scopes.rs
packages/functions/auth/tests/password_login_slice.rs
packages/functions/auth/tests/runtime_route_slice.rs
packages/functions/auth/tests/support/mod.rs
```

Legacy source files may remain compiled if they are not used by the target login route. This slice must not depend on `ui/password.rs`, `ui/select.rs`, or the legacy `provider/password.rs` hosted-form flow.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add typed authorize-session and authorization-code records with store tests.
2. Add HMAC key helpers for authorize sessions and authorization codes if the existing helpers are incomplete.
3. Cut `/authorize` over to typed authorize-session storage and `AUTH_AUTHORIZE_SESSION_TTL_SECONDS`.
4. Add OIDC `nonce` parsing and persistence on authorize sessions.
5. Add scope validation/normalization against configured client allowed scopes where missing.
6. Add password flow rate-limit tests for registration, verification, and login.
7. Wire rate limiting into the existing registration and verification routes.
8. Add password login domain tests for correct password, wrong password, unverified email, unknown email, missing subject, and inactive account.
9. Implement password login domain logic.
10. Add `POST /password/login` route returning `303 See Other`.
11. Add route tests proving successful login redirects with `code` and `state`.
12. Add route tests proving login responses contain no access token, refresh token, or ID token.
13. Add route/store tests proving raw session keys and raw authorization codes do not appear in storage keys.
14. Run full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Store Tests

- `create_authorize_session` stores by HMAC digest, not raw session key.
- `take_authorize_session` consumes once.
- Expired authorize session is rejected.
- Authorize session record stores `expires_at` and DynamoDB TTL.
- `create_authorization_code` stores by HMAC digest, not raw code.
- Authorization code record stores `expires_at` and DynamoDB TTL.
- Authorization code record preserves client ID, redirect URI, subject, scope, PKCE challenge, and OIDC nonce.

### Authorize Tests

- `/authorize` creates a typed authorize session.
- `/authorize` accepts `nonce` only as stored client OIDC nonce metadata.
- `/authorize` rejects unsupported `code_challenge_method`.
- `/authorize` rejects unknown scopes.
- `/authorize` rejects scopes not allowed by the configured client.
- `/authorize` with `provider=password` does not render hosted UI.
- `/authorize` does not issue authorization codes or tokens.

### Password Login Domain Tests

- Verified user with correct password and active account receives an authorization-code redirect URI.
- Wrong password fails without consuming the authorize session.
- Unknown email fails without revealing whether the email exists.
- Unverified password user fails without creating an authorization code.
- Password user missing subject fails.
- Deleted account fails.
- Disabled account fails if disabled status is introduced in this slice.
- Successful login consumes the authorize session.
- Reusing the same session fails.
- Successful login creates an authorization code record with HMAC lookup key.

### Route Tests

- `POST /password/register` is rate-limited.
- `POST /password/verify` is rate-limited.
- `POST /password/login` is rate-limited.
- Password rate-limit keys do not contain raw email addresses, passwords, session keys, verification tokens, or authorization codes.
- `POST /password/login` with valid form data returns `303 See Other`.
- Redirect location is the registered redirect URI.
- Redirect location includes `code`.
- Redirect location includes original OAuth `state`.
- Response does not include access token, refresh token, or ID token.
- Invalid credentials return a safe failure.
- Unverified users cannot receive codes.
- Expired or reused sessions cannot receive codes.

## Acceptance Criteria

- Password login is the first password flow that can create an OAuth authorization code.
- Registration and verification still never issue authorization codes or tokens.
- Only verified password users with active accounts can receive authorization codes.
- Login verifies Argon2id password hashes.
- Login consumes a short-lived typed authorize session.
- Authorization codes are short-lived, single-purpose records keyed by HMAC digest.
- Raw passwords, raw session keys, and raw authorization codes are not stored in DynamoDB keys.
- Registration, verification, and login routes are rate-limited without storing raw password-flow secrets in rate-limit keys.
- Login returns an OAuth redirect, not tokens.
- Token exchange remains unchanged and incomplete for typed codes until slice 05.
- The auth Lambda remains API-only and renders no login UI.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

Manual protocol smoke test should be done only after the slice is implemented:

```text
GET /authorize?response_type=code&client_id=web&redirect_uri=<registered>&state=abc&scope=openid%20email&provider=password&code_challenge=<challenge>&code_challenge_method=S256
POST /password/login
```

Expected result:

- `/authorize` creates a short-lived session without tokens.
- `/password/login` redirects to the registered callback with `code` and `state`.
- DynamoDB contains no raw session key or raw authorization code in `pk` or `sk`.

AWS validation should wait until after slice 05 for full code-to-token exchange. Slice 04 can be smoke-tested up to redirect-code issuance only.

## Next Slice

After this slice, implement `05_token_exchange_refresh_userinfo_and_logout`.

That slice should:

- consume typed authorization codes
- validate PKCE at token exchange
- issue JWT access tokens
- issue OIDC ID tokens when `openid` is granted
- create and rotate refresh tokens
- add user-facing refresh-token revocation for logout
- cut `/userinfo` over to the new account/identity model
