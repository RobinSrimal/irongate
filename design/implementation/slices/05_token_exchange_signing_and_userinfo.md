# 05_token_exchange_signing_and_userinfo

## Goal

Cut the target `authorization_code` token exchange path over to typed authorization-code storage, the configured runtime signer, and runtime-signature userinfo.

At the end of this slice, a client can complete the first password-backed OAuth/OIDC loop:

```text
/authorize -> /password/login -> authorization code -> /token -> access token + optional ID token -> /userinfo
```

Refresh-token issuance, refresh rotation, refresh-token reuse detection, and user-facing logout stay out of this slice. That keeps the code change surface small enough to review and test properly.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/core/tokens.md`
- `design/auth/crypto/signing.md`
- `design/auth/store/authorization-codes.md`
- `design/auth/store/accounts.md`
- `design/auth/store/identities.md`
- `design/auth/store/keys.md`
- `design/auth/observability/audit.md`
- `design/scope.md`

This slice intentionally implements only the `authorization_code` subset of `design/auth/api/oauth/token.md`. The `refresh_token` grant and `POST /oauth/revoke` are deferred to slice 06. Discovery metadata must advertise only the behavior implemented at the end of this slice.

## Why This Slice Next

Slice 04 created the first target authentication boundary:

```text
verified password user + valid authorize session -> typed authorization code
```

The next useful boundary is:

```text
typed authorization code + PKCE verifier -> signed access token + optional signed ID token
```

This also fixes the most important remaining signing mismatch from earlier reviews: runtime configuration already loads a configured signer, but legacy token/JWKS/userinfo paths still use the old DynamoDB signing-key path. This slice should remove that mismatch before refresh tokens make the token surface larger.

## In Scope

### Runtime Signer And JWKS Cutover

Cut public-key metadata and target token signing over to the configured runtime signer.

Required behavior:

- `GET /.well-known/jwks.json` returns public keys from the configured runtime signer.
- Target access-token signing uses the configured runtime signer.
- Target ID-token signing uses the configured runtime signer.
- Target access-token verification for `/userinfo` uses the same runtime public key material.
- The target token, JWKS, and userinfo paths do not call `jwt::keys::get_or_create_signing_key`, `jwt::keys::get_all_signing_keys`, or any storage-backed `signing:key` path.
- Private signing key material is never serialized through JWKS, discovery, token responses, logs, errors, or DynamoDB records.
- `AUTH_SIGNING_MODE=local-es256` is the only implemented signing mode for this slice.
- `AUTH_SIGNING_MODE=kms-es256` may remain a startup error with the existing clear "not implemented" message.

This slice may keep the legacy signing modules compiled if other legacy code still references them. The target runtime paths listed above must not depend on them.

### Access-Token Audience Configuration

Access-token audience must be explicit and must not be confused with the OAuth client ID used as the ID-token audience.

Add or wire a minimal runtime setting:

```text
AUTH_ACCESS_TOKEN_AUDIENCE
```

Rules:

- Access-token `aud` is `AUTH_ACCESS_TOKEN_AUDIENCE`.
- ID-token `aud` is the OAuth `client_id`.
- If the environment value is absent, use the configured issuer URL as the default auth-service audience.
- Tests must prove access-token `aud` and ID-token `aud` are different when `AUTH_ACCESS_TOKEN_AUDIENCE` differs from `client_id`.

This is intentionally a single global audience for now. Per-client or resource-indicator audience policy is a later design decision.

### Token Claim Shape

Add or centralize token claim construction in the target token/core modules.

Access token claims:

```json
{
  "mode": "access",
  "iss": "https://auth.example.com",
  "sub": "user_...",
  "aud": "https://api.example.com",
  "iat": 123,
  "exp": 456,
  "scope": "openid email",
  "subject_type": "user",
  "properties": {
    "email": "user@example.com",
    "email_verified": true,
    "provider": "password"
  }
}
```

ID token claims:

```json
{
  "mode": "id",
  "iss": "https://auth.example.com",
  "sub": "user_...",
  "aud": "web",
  "iat": 123,
  "exp": 456,
  "nonce": "optional",
  "email": "optional",
  "email_verified": "optional"
}
```

Rules:

- Access tokens are self-contained ES256 JWTs.
- ID tokens are self-contained ES256 JWTs.
- Access tokens are not persisted.
- ID tokens are not persisted.
- ID tokens are issued only when the granted scope includes `openid`.
- The initial ID-token `nonce` comes from the stored authorize request nonce.
- `email` and `email_verified` claims are included only when the granted scope includes `email` and the authorization-code properties contain those values.
- Profile claims are not added unless there is an explicit profile claim source; do not invent profile data.
- Token TTLs come from `AUTH_ACCESS_TOKEN_TTL_SECONDS` and `AUTH_ID_TOKEN_TTL_SECONDS`.

### Authorization-Code Token Exchange

Cut the `authorization_code` branch of `POST /token` over to typed authorization-code consumption.

Required request shape:

```text
grant_type=authorization_code
client_id=web
code=<raw authorization code>
redirect_uri=https://app.example.com/auth/callback
code_verifier=<raw PKCE verifier>
```

Required behavior:

1. Parse `application/x-www-form-urlencoded` requests.
2. Resolve the OAuth client from the config-only client registry.
3. Authenticate confidential clients with their configured token endpoint auth method.
4. Validate public clients without requiring a client secret.
5. Require the client to be allowed to use `authorization_code`.
6. Compute the authorization-code HMAC lookup digest.
7. Consume the code with `AuthStore::take_authorization_code`.
8. Reject missing, expired, or already consumed codes with a safe `invalid_grant`.
9. Require stored `client_id` to match the requesting client.
10. Require stored `redirect_uri` to match the submitted redirect URI exactly.
11. Require PKCE for clients that require PKCE.
12. Validate the submitted verifier against the stored S256 challenge.
13. Require the stored subject account to still be active before issuing tokens.
14. Issue an access token.
15. Issue an ID token only when the stored scope contains `openid`.
16. Return JSON with no refresh token in this slice.

Response shape:

```json
{
  "access_token": "...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "scope": "openid email",
  "id_token": "optional"
}
```

`refresh_token` must not appear in the response during this slice.

### Offline Access Boundary

This slice does not implement refresh-token storage or refresh-token grants.

Rules:

- `grant_type=refresh_token` returns `unsupported_grant_type` or the existing OAuth equivalent until slice 06.
- `offline_access` must not be advertised in discovery metadata at the end of this slice.
- If a submitted authorization code carries `offline_access`, the token endpoint must not silently issue a response that implies refresh support.
- The preferred behavior is to reject that exchange with `invalid_scope` until slice 06 adds refresh-token issuance.

This keeps metadata and runtime behavior honest while allowing the static sample client config to keep its eventual v1 grants and scopes.

### Userinfo Cutover

Cut `GET /userinfo` over to runtime-signature access-token verification.

Required behavior:

- Accept `Authorization: Bearer <access_token>`.
- Verify ES256 signature using runtime signer public material.
- Verify `iss`.
- Verify `aud` against `AUTH_ACCESS_TOKEN_AUDIENCE`.
- Verify `exp`.
- Verify token `mode=access`.
- Reject ID tokens.
- Require the subject account to still be active.
- Return only intended claims.

Response shape:

```json
{
  "sub": "user_...",
  "type": "user",
  "email": "user@example.com",
  "email_verified": true
}
```

Rules:

- `email` and `email_verified` are returned only when the access token has `email` scope and those properties are available.
- Do not return refresh-token state, authorization-code data, internal storage keys, password hashes, provider tokens, or signing key data.
- Userinfo remains an account-status-aware endpoint; it is not token introspection.

### Discovery Metadata

Metadata must match implemented behavior at the end of this slice.

Required behavior for this slice:

- Keep `authorization_endpoint`.
- Keep `token_endpoint`.
- Keep `userinfo_endpoint` only after userinfo is cut over to runtime signer verification.
- Keep `jwks_uri`.
- Advertise `response_types_supported = ["code"]`.
- Advertise `grant_types_supported = ["authorization_code"]`.
- Advertise `id_token_signing_alg_values_supported = ["ES256"]`.
- Advertise `code_challenge_methods_supported = ["S256"]`.
- Do not advertise `revocation_endpoint`.
- Do not advertise `introspection_endpoint`.
- Do not advertise `refresh_token` grant until slice 06 implements typed refresh rotation.
- Do not advertise `offline_access` until slice 06 implements refresh-token issuance.

After slice 06, discovery can be expanded to the full v1 target described in `design/auth/api/oauth/discovery.md`.

### Audit Events

Emit sanitized audit events for token exchange.

Required events:

- `authorization_code_exchanged` on successful code exchange.
- `authorization_code_exchange_failed` or an equivalent safe event on failed exchange.
- Existing login events from earlier slices remain unchanged.

Rules:

- Do not log raw authorization codes.
- Do not log raw access tokens.
- Do not log raw ID tokens.
- Do not log PKCE verifiers.
- Token/code references, if needed, use safe hashes only.
- Audit mode still follows `AUTH_AUDIT_LOG_MODE`.

## Out Of Scope

- Refresh-token issuance.
- Refresh-token storage.
- `grant_type=refresh_token`.
- Refresh-token rotation.
- Refresh-token reuse detection.
- `POST /oauth/revoke`.
- User-facing logout.
- Account lifecycle admin routes.
- Google or Apple login.
- Password reset.
- KMS ES256 implementation.
- Token introspection.
- Opaque access tokens.
- Generic OAuth/OIDC provider support.
- Hosted UI, consent UI, or account-selection UI.

## Expected Code Shape

Current repo paths should follow the intended design tree and avoid reintroducing `flows`.

Target modules:

```text
packages/functions/auth/src/api/oauth/token.rs
packages/functions/auth/src/api/oauth/userinfo.rs
packages/functions/auth/src/api/oauth/discovery.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/oauth/userinfo.rs
packages/functions/auth/src/oauth/well_known.rs
packages/functions/auth/src/crypto/signing.rs
packages/functions/auth/src/core/tokens.rs
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/config/ttls.rs
packages/functions/auth/src/store/authorization_codes.rs
packages/functions/auth/src/store/accounts.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/tests/token_exchange_slice.rs
packages/functions/auth/tests/runtime_route_slice.rs
packages/functions/auth/tests/foundation_slice.rs
packages/functions/auth/tests/support/mod.rs
```

If the current implementation still keeps handler code under `oauth/*.rs`, the slice may either:

- add thin `api/oauth/*.rs` modules and move target handlers there, or
- keep the existing `oauth/*.rs` files only as re-export/wrapper boundaries while putting new target behavior in `api/oauth/*.rs`.

Do not add new behavior under legacy `provider/*`, `ui/*`, or `flows/*`.

Legacy modules may remain compiled if removing them would enlarge this slice. The target token, JWKS, and userinfo routes must not call legacy DynamoDB signing-key code.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing tests proving JWKS currently comes from the runtime signer and does not create or read `signing:key` records.
2. Add failing discovery tests for this slice's metadata: `authorization_code` only, no `refresh_token` grant, no `revocation_endpoint`, no `offline_access`, no introspection endpoint.
3. Add failing token claim tests for access-token audience, ID-token audience, ID-token nonce, and email-scope behavior.
4. Add failing authorization-code exchange tests for successful code exchange, ID-token issuance, no refresh token, and single-use code consumption.
5. Add failing negative exchange tests for code replay, expired code, client mismatch, redirect mismatch, missing verifier, PKCE mismatch, inactive account, and `offline_access` scope.
6. Add failing userinfo tests for valid access token, ID-token rejection, wrong audience rejection, expired token rejection, inactive account rejection, and email-scope filtering.
7. Add or wire `AUTH_ACCESS_TOKEN_AUDIENCE` in runtime config with issuer URL as the default.
8. Extend `crypto/signing.rs` so the runtime signer can sign access-token and ID-token claims without exposing private key material outside the signer boundary.
9. Add `core/tokens.rs` claim structs and constructors for access tokens and ID tokens.
10. Cut JWKS over to `state.runtime.signer.jwks()`.
11. Update discovery metadata to advertise only this slice's implemented behavior.
12. Cut `POST /token` `authorization_code` handling over to typed `AuthStore::take_authorization_code`.
13. Add PKCE S256 validation against the typed authorization-code record.
14. Add `require_active_account` before signing tokens.
15. Sign and return access tokens with `AUTH_ACCESS_TOKEN_AUDIENCE`.
16. Sign and return ID tokens only for `openid` scope, preserving the stored authorize nonce.
17. Ensure token responses do not include refresh tokens in this slice.
18. Cut `/userinfo` over to runtime signer verification and active-account checks.
19. Emit sanitized token-exchange audit events without raw secrets.
20. Remove target route dependencies on `jwt::keys` storage-backed signing helpers.
21. Run focused Rust tests.
22. Run full Rust tests.
23. Run `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Signing And JWKS Tests

- `/.well-known/jwks.json` returns the runtime signer `kid`.
- JWKS contains public key material only.
- JWKS does not create a `signing:key` record.
- JWKS does not read from `signing:key` records.
- Access-token signing uses the runtime signer `kid`.
- ID-token signing uses the runtime signer `kid`.

### Discovery Tests

- OpenID metadata advertises `authorization_code`.
- OpenID metadata does not advertise `refresh_token`.
- OpenID metadata does not advertise `offline_access`.
- OpenID metadata does not advertise `revocation_endpoint`.
- OpenID metadata does not advertise `introspection_endpoint`.
- OAuth metadata matches the same implemented grant and endpoint set.

### Token Exchange Tests

- Valid typed authorization code returns `access_token`.
- Valid typed authorization code with `openid` scope returns `id_token`.
- Valid typed authorization code without `openid` scope does not return `id_token`.
- Token response never returns `refresh_token` in this slice.
- Token response includes `token_type=Bearer`.
- Token response `expires_in` matches `AUTH_ACCESS_TOKEN_TTL_SECONDS`.
- Token response `scope` matches the granted scope, excluding unsupported `offline_access`.
- Consumed authorization code cannot be reused.
- Raw authorization code is not used as a DynamoDB key during exchange.
- Unknown code returns safe `invalid_grant`.
- Expired code returns safe `invalid_grant`.
- Client mismatch returns safe `invalid_grant`.
- Redirect URI mismatch returns safe `invalid_grant`.
- Missing PKCE verifier fails.
- Wrong PKCE verifier fails.
- Unsupported `code_challenge_method` remains rejected before code issuance.
- Deleted account cannot exchange a code for tokens.
- Disabled account fails if disabled status has been introduced by the time this slice runs.
- Code carrying `offline_access` is rejected until slice 06 implements refresh-token issuance.

### Token Claim Tests

- Access token validates with ES256, expected issuer, expected `kid`, and `AUTH_ACCESS_TOKEN_AUDIENCE`.
- Access token `aud` is not the OAuth client ID when an explicit access-token audience is configured.
- Access token includes stable `sub`.
- Access token includes granted `scope`.
- Access token includes minimal `properties`.
- ID token validates with ES256, expected issuer, expected `kid`, and OAuth `client_id` as `aud`.
- ID token includes stored `nonce` from `/authorize`.
- ID token includes `email` and `email_verified` only when `email` scope was granted.
- ID token omits `nonce` when no nonce was supplied.

### Userinfo Tests

- Valid access token returns `sub` and subject type.
- Valid access token with `email` scope returns `email` and `email_verified`.
- Valid access token without `email` scope does not return email claims.
- ID token is rejected by `/userinfo`.
- Access token with wrong issuer is rejected.
- Access token with wrong audience is rejected.
- Expired access token is rejected.
- Deleted account is rejected even if the access token signature is valid.
- Disabled account is rejected if disabled status has been introduced by the time this slice runs.
- Userinfo response does not include password hash, refresh-token state, auth-code data, internal keys, or signing data.

### Audit And Secret Handling Tests

- Successful code exchange emits a sanitized audit event when audit logging is enabled.
- Failed code exchange emits a sanitized audit event when audit logging is enabled.
- Audit events do not contain raw authorization codes, access tokens, ID tokens, PKCE verifiers, or private keys.
- Token and userinfo errors do not include raw tokens, authorization codes, PKCE verifiers, or storage digests.

## Acceptance Criteria

- `/token` supports typed `authorization_code` exchange for the target password login path.
- `/token` does not use legacy raw `oauth:code` storage.
- Authorization codes are consumed once through typed store operations.
- PKCE is enforced at token exchange.
- Token issuance requires an active account.
- Access tokens are signed ES256 JWTs through the configured runtime signer.
- ID tokens are signed ES256 JWTs through the configured runtime signer.
- ID tokens are issued only when `openid` was granted.
- Access-token `aud` and ID-token `aud` are not conflated.
- JWKS exposes only runtime signer public key material.
- `/userinfo` verifies runtime-signed access tokens and rejects ID tokens.
- `/userinfo` checks account status before returning claims.
- Discovery metadata advertises only implemented behavior for this slice.
- No target token, JWKS, or userinfo path reads or writes plaintext JWT private keys in `AuthTable`.
- No refresh token is issued, stored, rotated, or revoked in this slice.
- `/oauth/revoke` is still not mounted or advertised.
- Access tokens, ID tokens, authorization codes, and PKCE verifiers are not logged or stored in DynamoDB keys.

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
GET /authorize?response_type=code&client_id=web&redirect_uri=<registered>&state=abc&scope=openid%20email&provider=password&code_challenge=<challenge>&code_challenge_method=S256&nonce=n-123
POST /password/login
POST /token grant_type=authorization_code client_id=web code=<code> redirect_uri=<registered> code_verifier=<verifier>
GET /userinfo Authorization: Bearer <access_token>
GET /.well-known/jwks.json
GET /.well-known/openid-configuration
```

Expected result:

- `/token` returns an access token.
- `/token` returns an ID token when `openid` was granted.
- `/token` does not return a refresh token.
- `/userinfo` accepts the access token and returns only intended claims.
- JWKS contains the runtime signer public key.
- Discovery does not advertise refresh or revocation yet.
- DynamoDB contains no raw authorization code in `pk` or `sk`.
- DynamoDB contains no target runtime signing private key record.

AWS validation can wait until after slice 06 or the AWS hardening slice. This slice can be validated locally because it does not depend on API Gateway source-IP behavior or KMS.

## Next Slice

After this slice, implement `06_refresh_rotation_and_logout`.

That slice should:

- issue refresh tokens when `offline_access` policy is satisfied
- store refresh tokens by HMAC lookup digest
- support `grant_type=refresh_token`
- rotate refresh tokens atomically
- detect refresh-token reuse
- add user-facing `POST /oauth/revoke`
- advertise `refresh_token`, `offline_access`, and `revocation_endpoint` only after they work
