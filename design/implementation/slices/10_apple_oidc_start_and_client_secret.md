# 10_apple_oidc_start_and_client_secret

## Goal

Add the first half of Sign in with Apple support: runtime Apple configuration, Apple client-secret JWT generation, `/authorize provider=apple` handoff, and an API-only Apple authorization redirect.

At the end of this slice, an application can start a normal OAuth authorize request with `provider=apple`. The auth Lambda creates a typed authorize session, redirects to `/apple/authorize?session=...`, creates a short-lived HMAC-keyed Apple provider-state record, and redirects the browser to Apple with `state`, `nonce`, and PKCE.

This slice intentionally stops before the Apple callback. The next slice validates Apple ID tokens, maps Apple `issuer + sub` to an internal identity, and creates the internal OAuth authorization code.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/oauth/authorize.md`
- `design/auth/api/providers/apple.md`
- `design/auth/providers/apple.md`
- `design/auth/store/authorize-sessions.md`
- `design/auth/store/provider-states.md`
- `design/auth/store/keys.md`
- `design/auth/config/environment.md`
- `design/infra/secrets.md`
- `design/scope.md`

The important design constraint is that Apple is first-class OIDC provider support, not a generic OAuth2 provider registry. This slice must not reintroduce hosted provider-selection UI, generic OAuth2 identity providers, raw provider state keys, or legacy provider state storage.

## Why This Slice Next

Google OIDC now works end to end. Apple is the next external identity provider in scope, but it has an extra security-sensitive component: the auth server must generate a client-secret JWT from Apple private key material.

Splitting Apple into two slices keeps this step manageable:

```text
/authorize provider=apple -> typed authorize session -> typed provider state -> Apple redirect
```

The next slice can then focus on the callback boundary:

```text
Apple callback code+state -> validate Apple ID token -> issue internal authorization code
```

## In Scope

### Apple Runtime Configuration

Add target Apple configuration to runtime auth config.

Target code:

```text
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/config/apple.rs
```

Runtime variables:

```text
AUTH_APPLE_CLIENT_ID optional set with the other AUTH_APPLE_* values
AUTH_APPLE_TEAM_ID optional set with the other AUTH_APPLE_* values
AUTH_APPLE_KEY_ID optional set with the other AUTH_APPLE_* values
AUTH_APPLE_PRIVATE_KEY_SECRET optional set with the other AUTH_APPLE_* values
AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS optional, default 86400
```

`AUTH_APPLE_PRIVATE_KEY_SECRET` is a secret reference name. The actual private key value is resolved through the existing runtime secret resolver, the same way local signing key material is resolved.

Rules:

- If all Apple variables are absent, Apple is disabled and password/Google deployments still start.
- If any Apple variable is present, all required Apple variables must be present.
- Startup fails clearly if the secret reference cannot be resolved.
- Startup fails if the private key is not valid ES256/P-256 private key material.
- Startup fails if `AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS` is outside the allowed range.
- Apple authorization, token, issuer, and JWKS URLs are fixed constants in code.
- Apple private key material is not logged, not stored in DynamoDB, not exposed through debug output, and not committed to config files.
- Deployed stages should supply the private key through SST secrets.

Fixed Apple values:

```text
authorization_url = https://appleid.apple.com/auth/authorize
token_url = https://appleid.apple.com/auth/token
issuer = https://appleid.apple.com
jwks_uri = https://appleid.apple.com/auth/keys
authorization_scopes = name email
```

### Apple Client-Secret JWT

Add Apple client-secret JWT generation.

Target code:

```text
packages/functions/auth/src/providers/apple.rs
```

The generated JWT should use:

```text
alg = ES256
kid = AUTH_APPLE_KEY_ID
iss = AUTH_APPLE_TEAM_ID
sub = AUTH_APPLE_CLIENT_ID
aud = https://appleid.apple.com
iat = current time
exp = iat + AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS
```

Rules:

- The generated client secret is used only for Apple token exchange in the next slice.
- The generated client secret must not be stored in DynamoDB.
- The generated client secret must not appear in logs, errors, route responses, or tests except as an in-memory test assertion.
- TTL must be bounded. Use a conservative default of one day and a maximum of 180 days because Apple client secrets must be time-bounded.
- Unit tests should verify the JWT header and claims and should verify the signature using the derived public key.

### Authorize Endpoint Provider Selection

Update `/authorize` to accept `provider=apple`.

Target code:

```text
packages/functions/auth/src/oauth/authorize.rs
```

Required behavior:

- Continue requiring an explicit provider.
- Continue accepting `provider=password` and `provider=google` with existing behavior.
- Accept `provider=apple` only when Apple is enabled in runtime config.
- Reject `provider=apple` with a safe protocol error when Apple is disabled.
- Store the typed authorize session with `selected_provider=apple`.
- Redirect to:

```text
/apple/authorize?session=<raw-authorize-session-key>
```

The endpoint must not redirect directly to Apple. Apple-specific state, nonce, and PKCE are owned by the Apple provider-start route.

### Apple Provider Start Route

Add an API-only Apple start route.

Target code:

```text
packages/functions/auth/src/api/providers/apple.rs
packages/functions/auth/src/providers/apple.rs
packages/functions/auth/src/routes.rs
```

Route:

```text
GET /apple/authorize?session=<raw-authorize-session-key>
```

Required behavior:

1. Require Apple to be enabled.
2. Require a `session` query parameter.
3. Compute the authorize-session lookup digest from the raw session key.
4. Verify the authorize session exists and has `selected_provider=apple`.
5. Generate raw Apple provider state.
6. Generate Apple OIDC nonce.
7. Generate Apple PKCE verifier and challenge.
8. Store provider state through `create_provider_state`.
9. Redirect to Apple authorization URL.

The Apple authorization URL must include:

```text
client_id
redirect_uri
response_type=code
response_mode=form_post
scope=name email
state=<raw-provider-state>
nonce=<raw-provider-nonce>
code_challenge=<S256 challenge>
code_challenge_method=S256
```

The callback redirect URI should be derived from the configured issuer URL:

```text
<issuer_url>/apple/callback
```

For dev/test fallback only, if issuer URL is absent, use the same existing localhost fallback convention as other OAuth endpoints.

### Apple Domain Helpers

Add small Apple-specific helpers rather than a broad provider abstraction.

Target code:

```text
packages/functions/auth/src/providers/apple.rs
```

Suggested helpers:

```text
AppleConfig
ApplePrivateKey
AppleClientSecretClaims
AppleAuthorizeInput
build_apple_authorization_url
apple_callback_uri
generate_apple_client_secret
```

Rules:

- URL construction is deterministic and unit-testable.
- Apple provider scopes are fixed for v1.
- Provider nonce is separate from the first-party OIDC client nonce stored on the authorize session.
- The helper does not exchange Apple codes or validate ID tokens in this slice.

### Provider State Reuse

Reuse the existing typed provider-state store from Google.

Provider-state records for Apple should use:

```json
{
  "session_lookup_digest": "...",
  "provider": "apple",
  "pkce_verifier": "...",
  "nonce": "...",
  "created_at": "...",
  "expires_at": "..."
}
```

Rules:

- Raw Apple provider state is generated with high entropy.
- DynamoDB keys use HMAC lookup digests, never raw provider state.
- Records carry `expires_at`.
- DynamoDB TTL uses the same expiry value.
- Provider states are single-use.
- Expired provider states are rejected even if DynamoDB TTL has not deleted them.
- The record stores the authorize-session lookup digest, not the raw authorize-session key.
- Routes and providers must not use generic `set` or `get` for provider-state records.

## Out Of Scope

- `POST /apple/callback`.
- Apple code exchange.
- Apple ID-token validation.
- Apple JWKS fetching or caching.
- Apple identity persistence.
- Authorization-code issuance after Apple proof.
- Account linking between password, Google, and Apple identities.
- Generic OAuth2 or generic OIDC provider support.
- Hosted provider-selection UI.
- Refresh-token issuance changes.
- Live Apple developer account validation.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree.

Target modules:

```text
packages/functions/auth/src/api/providers/apple.rs
packages/functions/auth/src/api/providers/mod.rs
packages/functions/auth/src/providers/apple.rs
packages/functions/auth/src/providers/mod.rs
packages/functions/auth/src/config/apple.rs
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/oauth/authorize.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/store/provider_states.rs
packages/functions/auth/src/store/authorize_sessions.rs
packages/functions/auth/tests/apple_oidc_start_slice.rs
packages/functions/auth/tests/startup_config_slice.rs
```

Legacy `packages/functions/auth/src/provider/apple.rs`, `provider/oidc.rs`, and generic provider routes may remain compiled if they are not used by the target Apple route. This slice must not depend on legacy `ProviderConfig::Oidc` or raw `provider:state` writes.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing config tests for Apple disabled, enabled, half-configured, missing private-key secret, invalid private key, and redacted debug output.
2. Implement `config/apple.rs` and add it to `RuntimeAuthConfig`.
3. Add failing Apple client-secret JWT tests for header, claims, TTL, and signature verification.
4. Implement Apple client-secret generation in `providers/apple.rs`.
5. Add failing Apple authorization URL-builder tests proving URL parameters, scopes, response mode, nonce, state, and PKCE challenge.
6. Implement Apple URL-building helpers.
7. Add failing `/authorize provider=apple` route tests.
8. Update `/authorize` to accept Apple only when enabled and redirect to `/apple/authorize`.
9. Add failing `GET /apple/authorize` route tests.
10. Implement `GET /apple/authorize`.
11. Add tests proving raw Apple provider state and raw authorize-session key do not appear in DynamoDB keys.
12. Update docs if implementation decisions refine Apple config or secret names.
13. Run full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Config Tests

- Runtime config starts with Apple disabled when all Apple variables are absent.
- Runtime config enables Apple when all required Apple variables and private key secret are present.
- Runtime config fails when only one Apple variable is present.
- Runtime config fails when `AUTH_APPLE_PRIVATE_KEY_SECRET` does not resolve.
- Runtime config fails when the resolved private key is invalid.
- Runtime config rejects invalid Apple client-secret TTL values.
- Runtime config debug output does not expose the Apple private key.

### Apple Client-Secret Tests

- Generated client secret uses ES256.
- JWT header contains configured Apple key ID.
- Claims contain configured team ID as `iss`.
- Claims contain configured client ID as `sub`.
- Claims contain `https://appleid.apple.com` as `aud`.
- Claims contain bounded `iat` and `exp`.
- Signature verifies against the configured Apple private key's public key.
- Debug output and errors do not expose private key material.

### Apple URL Tests

- Apple authorization URL uses `https://appleid.apple.com/auth/authorize`.
- URL includes configured Apple client ID.
- URL includes callback URI `<issuer_url>/apple/callback`.
- URL includes `response_type=code`.
- URL includes `response_mode=form_post`.
- URL includes `scope=name email`.
- URL includes raw state.
- URL includes provider nonce.
- URL includes S256 PKCE challenge and method.
- URL does not include Apple private key material or generated client secret.

### Route Tests

- `/authorize` rejects `provider=apple` when Apple is disabled.
- `/authorize` accepts `provider=apple` when Apple is enabled.
- `/authorize` with Apple creates a typed authorize session with `selected_provider=apple`.
- `/authorize` with Apple redirects to `/apple/authorize?session=...`, not directly to Apple.
- `GET /apple/authorize` redirects to Apple.
- `GET /apple/authorize` creates one typed provider-state record with `provider=apple`.
- `GET /apple/authorize` rejects missing session.
- `GET /apple/authorize` rejects a session whose stored `selected_provider` is not `apple`.
- Route responses and stored keys do not expose raw provider state, raw authorize-session key, Apple private key material, generated client secret, access tokens, refresh tokens, or ID tokens.

## Acceptance Criteria

- Password-only and Google-only deployments still work without Apple credentials.
- Half-configured Apple credentials fail startup.
- Apple private key material is resolved through a secret reference and redacted from debug/errors.
- Apple client-secret JWT generation is tested and does not persist secrets.
- Apple start uses fixed first-class Apple OIDC configuration, not generic provider config.
- `/authorize provider=apple` works only when Apple is enabled.
- `/apple/authorize` creates typed provider state with HMAC lookup key and TTL.
- Raw Apple provider state is only sent to Apple, not stored in DynamoDB keys.
- Raw authorize-session key is not stored in provider-state records or keys.
- Apple authorization URL includes state, nonce, PKCE, and `response_mode=form_post`.
- No Apple callback, token exchange, ID-token validation, identity persistence, or internal authorization-code issuance is implemented in this slice.
- Existing password, Google, token exchange, refresh, and reset tests continue to pass.
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
GET /authorize?response_type=code&client_id=web&redirect_uri=<registered>&state=abc&scope=openid%20email&provider=apple&code_challenge=<challenge>&code_challenge_method=S256
GET /apple/authorize?session=<raw-session-from-authorize-redirect>
```

Expected result:

- `/authorize` creates a typed authorize session and redirects to `/apple/authorize`.
- `/apple/authorize` creates typed provider state and redirects to Apple.
- DynamoDB contains no raw provider state or raw authorize-session key in `pk` or `sk`.
- No Apple private key material or generated client-secret JWT is stored in DynamoDB.

Live Apple callback validation should wait for the next slice.

## Next Slice

After this slice, implement `11_apple_oidc_callback_and_identity`.

That slice should:

- add `POST /apple/callback`
- exchange the Apple authorization code using the generated Apple client-secret JWT
- validate Apple ID token issuer, audience, signature, expiry, and nonce
- map `https://appleid.apple.com` plus Apple `sub` to an internal subject
- create or reuse persisted Apple identity records
- require active accounts
- create a typed internal OAuth authorization code
- redirect back to the registered OAuth client callback
