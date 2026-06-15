# 06_refresh_rotation_and_logout

## Goal

Add long-lived session support on top of the runtime-signed authorization-code exchange from slice 05.

At the end of this slice, a client can request `offline_access`, receive a refresh token, rotate that refresh token through `grant_type=refresh_token`, and revoke the current refresh-token family through `POST /oauth/revoke` for ordinary app logout.

The slice must keep access tokens self-contained and short-lived. Logout revokes future refreshes only; it does not revoke already-issued access JWTs.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/core/tokens.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/store/accounts.md`
- `design/auth/store/keys.md`
- `design/auth/observability/audit.md`
- `design/scope.md`

This slice intentionally implements the refresh-token subset that was deferred from slice 05. It must not reintroduce opaque access tokens, token introspection, runtime client management, hosted UI, or admin lifecycle routes.

## Why This Slice Next

Slice 05 completed this boundary:

```text
typed authorization code + PKCE verifier -> runtime-signed access token + optional ID token
```

The next useful boundary is:

```text
offline access authorization -> opaque refresh token -> atomic rotation -> logout revocation
```

This is the point where the auth foundation becomes practical for normal web and mobile sessions without weakening the design decisions already made:

- access tokens remain stateless JWTs
- refresh tokens stay server-side revocable
- raw bearer secrets never become DynamoDB keys
- discovery advertises refresh and revocation only after they work

## In Scope

### Refresh Token Model

Refresh tokens in this slice are opaque high-entropy bearer secrets, not JWTs.

The raw refresh token is returned to the client only in token responses. DynamoDB stores only HMAC lookup digests and metadata needed to rotate or revoke the token.

Digest construction:

```text
refresh_lookup_digest = HMAC-SHA256(storage_lookup_secret, refresh_token)
```

Raw refresh tokens must not appear in:

- DynamoDB `pk`
- DynamoDB `sk`
- audit logs
- application logs
- error responses
- token metadata records

Recommended raw token format:

```text
ig_rt_<random_urlsafe_secret>
```

The prefix is for operator/debug readability only. It is not security-sensitive and must not be used as the lookup key.

### Refresh Token Records

Add typed refresh-token storage that is purpose-built for rotation and revocation.

Target code:

```text
packages/functions/auth/src/store/refresh.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/keys.rs
```

Primary refresh token record:

```json
{
  "refresh_digest": "...",
  "family_id": "rtf_...",
  "client_id": "web",
  "subject": "user_...",
  "subject_type": "user",
  "scope": "openid email offline_access",
  "properties": {
    "email": "user@example.com",
    "email_verified": true,
    "provider": "password"
  },
  "issued_at": "...",
  "expires_at": "...",
  "last_used_at": "optional",
  "replaced_by": "optional refresh digest",
  "revoked_at": "optional"
}
```

Family/session record:

```json
{
  "family_id": "rtf_...",
  "client_id": "web",
  "subject": "user_...",
  "created_at": "...",
  "expires_at": "...",
  "revoked_at": "optional",
  "revoked_reason": "optional"
}
```

Index record:

```json
{
  "refresh_digest": "...",
  "family_id": "rtf_...",
  "client_id": "web",
  "subject": "user_...",
  "expires_at": "..."
}
```

Rules:

- Primary refresh records use refresh-token expiry as DynamoDB TTL.
- Family records use the same expiry as DynamoDB TTL.
- Subject/client index records use the same expiry as DynamoDB TTL.
- Rotation preserves the original family ID.
- A family-level revocation blocks every token in that family without scanning.
- Index records are created now so future admin lifecycle routes can revoke by subject without table-wide scans.
- Subject-wide admin revocation itself remains out of scope for this slice.

### Store Keys

Extend typed key helpers.

Required helpers:

```text
refresh_token(refresh_digest)
refresh_family(family_id)
refresh_by_subject(subject, refresh_digest)
refresh_by_client(client_id, refresh_digest)
```

The physical key strings may follow the current local convention, but they must preserve the logical design:

```text
refresh_token(refresh_digest)          -> exact lookup by HMAC digest
refresh_family(family_id)              -> exact lookup by generated family ID
refresh_by_subject(subject, digest)    -> bounded query support for a subject
refresh_by_client(client_id, digest)   -> bounded query support for a client
```

No raw refresh token may appear in any helper output.

### Store Operations

Add purpose-specific refresh store operations. Route code must not manipulate refresh records through generic `set`, `get`, `scan`, or ad hoc key arrays.

Required operations:

```text
create_refresh_token
rotate_refresh_token
revoke_refresh_token_family
get_refresh_token
```

`create_refresh_token`:

- accepts the token metadata from authorization-code exchange
- creates a new family ID
- creates a primary refresh record
- creates a family record
- creates subject/client index records
- uses conditional writes so token digest and family ID cannot collide
- returns the raw refresh token only to the caller

`rotate_refresh_token`:

- accepts a raw refresh token and requesting client ID
- computes the HMAC lookup digest
- loads the current refresh record
- verifies the token belongs to the requesting client
- verifies the refresh family is not revoked
- verifies the subject account is still active
- detects reuse when the token was already replaced or revoked
- creates a new raw refresh token and digest
- marks the old record as replaced by the new digest
- creates the new refresh record
- creates subject/client index records for the new digest
- performs old-token update and new-token creation in one transaction
- returns token metadata needed to issue new access tokens plus the new raw refresh token

`revoke_refresh_token_family`:

- accepts a raw refresh token and requesting client ID
- computes the HMAC lookup digest
- if the token is missing, invalid, expired, or already revoked, returns an idempotent success outcome
- if the token belongs to another client, returns the same idempotent success outcome without revoking anything
- if the token belongs to the requesting client, marks the family record revoked
- does not scan all refresh tokens in the family
- does not revoke other devices or all sessions for the subject

`get_refresh_token`:

- supports exact HMAC digest lookup for tests and route helpers
- rejects expired records based on stored `expires_at`

Future admin lifecycle work may add:

```text
revoke_refresh_tokens_for_subject
```

Do not implement that admin operation in this slice unless it is needed to keep the refresh store coherent.

### Authorization-Code Exchange With Offline Access

Extend the slice 05 authorization-code exchange behavior.

When the consumed authorization code scope does not include `offline_access`:

- issue access token
- issue ID token when `openid` was granted
- do not issue refresh token

When the consumed authorization code scope includes `offline_access`:

1. Require the OAuth client to be allowed to use the `refresh_token` grant.
2. Require the subject account to still be active.
3. Issue access token.
4. Issue ID token when `openid` was granted.
5. Create a refresh token through the typed refresh store.
6. Return the raw refresh token in the token response.

Response shape with refresh:

```json
{
  "access_token": "...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "ig_rt_...",
  "scope": "openid email offline_access",
  "id_token": "optional"
}
```

Rules:

- Refresh-token TTL comes from `AUTH_REFRESH_TOKEN_TTL_SECONDS`.
- Refresh-token issuance must not use the legacy storage-backed signing-key path.
- Refresh tokens are not signed JWTs in this slice.
- `offline_access` is allowed only after refresh issuance is implemented.
- Code exchange still consumes authorization codes once.

### Refresh Grant

Implement the target `refresh_token` branch of `POST /token`.

Request shape:

```text
grant_type=refresh_token
client_id=web
refresh_token=<raw refresh token>
```

Confidential clients authenticate the same way they do for authorization-code exchange. Public clients identify with `client_id`.

Successful response shape:

```json
{
  "access_token": "...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "ig_rt_new...",
  "scope": "openid email offline_access"
}
```

Rules:

- Refresh grant returns a new access token.
- Refresh grant returns a new refresh token on every successful use.
- Refresh grant does not return an ID token in this slice.
- New access-token claims use the original refresh record subject, subject type, scope, and properties.
- New access-token audience remains `AUTH_ACCESS_TOKEN_AUDIENCE`.
- Token issuance requires an active account.
- A deleted account cannot refresh.
- A disabled account must be rejected if disabled status exists by implementation time.
- A reused refresh token revokes the refresh family and fails with `invalid_grant`.
- Missing, expired, revoked, or wrong-client refresh tokens fail with safe errors that do not reveal raw token state.

### Refresh Token Reuse Detection

Reuse detection is mandatory for replaced or revoked refresh tokens.

Detection cases:

- Submitted refresh record has `replaced_by`.
- Submitted refresh record has `revoked_at`.
- Submitted refresh family has `revoked_at`.
- Atomic rotation condition fails and a reload shows the old token was replaced or revoked.

Required behavior:

- Emit a sanitized `refresh_token_reuse_detected` audit event.
- Revoke the refresh family if it is not already revoked.
- Return `invalid_grant`.
- Do not return the replacement digest or any token metadata to the caller.

Concurrent double-submit should be treated conservatively. If the old token is already replaced by the time the second request checks it, treat the second request as reuse and revoke the family.

### User-Facing Revoke Endpoint

Add the logout revocation route:

```text
POST /oauth/revoke
```

Target code:

```text
packages/functions/auth/src/api/oauth/revoke.rs
packages/functions/auth/src/oauth/revoke.rs
packages/functions/auth/src/routes.rs
```

Request shape:

```text
client_id=web
token=<raw refresh token>
token_type_hint=refresh_token
```

`token_type_hint` is optional. This endpoint supports refresh-token revocation only.

Client behavior:

- Confidential clients must authenticate with their configured token endpoint auth method.
- Public clients must identify with `client_id`.
- Public clients can revoke only refresh tokens that belong to the same client.
- If client authentication is invalid, return `invalid_client`.

Revocation behavior:

- If the token belongs to the requesting client, revoke that token family.
- If the token is missing, invalid, expired, already revoked, or belongs to a different client, return the same idempotent success response.
- Do not reveal whether the token existed.
- Do not revoke other clients' tokens.
- Do not revoke other devices.
- Do not revoke all sessions for the subject.
- Do not revoke already-issued access JWTs.

Response:

```text
HTTP 200
```

An empty response body is acceptable. A minimal JSON body is also acceptable if existing route conventions make that simpler, but it must not reveal token existence.

### Discovery Metadata

Update discovery only after refresh rotation and revoke routes work.

Required metadata after this slice:

```text
grant_types_supported = ["authorization_code", "refresh_token"]
scopes_supported includes "offline_access"
revocation_endpoint = "<issuer>/oauth/revoke"
```

Metadata must still not advertise:

```text
introspection_endpoint
client_credentials
opaque access tokens
unsupported signing algorithms
```

JWKS remains the runtime signer public key metadata from slice 05.

### Audit Events

Emit sanitized audit events for refresh behavior.

Required events:

- `refresh_token_issued`
- `refresh_token_rotated`
- `refresh_token_reuse_detected`
- `refresh_family_revoked`
- `user_logout_refresh_token_revoked`

Rules:

- Do not log raw refresh tokens.
- Do not log replacement refresh tokens.
- Do not log authorization codes.
- Do not log access tokens or ID tokens.
- Token references, if needed, use digests only.
- Audit mode still follows `AUTH_AUDIT_LOG_MODE`.

### Legacy Refresh Path Boundary

The current code still contains legacy refresh helpers that sign refresh tokens as JWTs and store raw refresh tokens under `oauth:refresh`.

Target routes in this slice must not call those legacy helpers.

Acceptable:

- Leave legacy helper code compiled if removing it would enlarge the slice.
- Keep old unit tests temporarily if they still compile and are not target route behavior.

Required:

- `POST /token` target refresh path uses typed refresh store operations.
- `POST /oauth/revoke` uses typed refresh store operations.
- No target route stores raw refresh tokens in DynamoDB keys.
- No target route scans `oauth:refresh` to revoke a user-facing session.

## Out Of Scope

- Google or Apple login.
- Password reset.
- IAM-protected account lifecycle admin routes.
- Subject-wide admin refresh revocation.
- Account disable/delete route implementation.
- KMS ES256 implementation.
- Token introspection.
- Opaque access tokens.
- Generic OAuth/OIDC provider support.
- Hosted UI, consent UI, or account-selection UI.
- Removal of every legacy refresh helper if it is not on a mounted target route.

## Expected Code Shape

Follow the intended design tree and current slice 05 API boundary.

Target modules:

```text
packages/functions/auth/src/api/oauth/token.rs
packages/functions/auth/src/api/oauth/revoke.rs
packages/functions/auth/src/api/oauth/discovery.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/oauth/revoke.rs
packages/functions/auth/src/core/tokens.rs
packages/functions/auth/src/store/refresh.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/tests/refresh_logout_slice.rs
packages/functions/auth/tests/token_exchange_slice.rs
packages/functions/auth/tests/support/mod.rs
```

Use `packages/functions/auth/src/store/refresh.rs` to match `design/auth/store/refresh-tokens.md`. Existing plural store modules may remain as-is; do not rename them in this slice.

The `api/oauth/*` modules should be the route-facing boundary. The lower-level `oauth/*` modules may keep implementation logic or become compatibility wrappers, but new route behavior should be reachable through `api/oauth/*`.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing store tests for refresh-token key construction: raw refresh token does not appear in primary, family, subject-index, or client-index keys.
2. Add failing store tests for `create_refresh_token`: primary record, family record, subject index, client index, expiry fields, and DynamoDB TTL are written.
3. Add failing store tests for `rotate_refresh_token`: old record is marked replaced, new token is created, new index records exist, raw tokens are never stored in keys.
4. Add failing store tests for reuse: submitting a replaced token revokes the family and returns a reuse outcome.
5. Add failing store tests for `revoke_refresh_token_family`: success, idempotent missing token, already revoked token, and wrong-client token.
6. Add failing route tests proving authorization-code exchange with `offline_access` returns a refresh token.
7. Add failing route tests proving authorization-code exchange without `offline_access` still omits refresh token.
8. Add failing route tests proving refresh grant rotates the token and returns a new access token plus new refresh token.
9. Add failing route tests proving the old refresh token cannot be reused.
10. Add failing route tests proving refresh after family revocation fails.
11. Add failing route tests proving deleted account cannot refresh.
12. Add failing route tests proving `POST /oauth/revoke` revokes the current family and is idempotent.
13. Add failing route tests proving one client cannot revoke another client's refresh token.
14. Add failing discovery tests for `refresh_token`, `offline_access`, and `revocation_endpoint`.
15. Add failing audit tests proving refresh events do not contain raw refresh tokens.
16. Extend `StoreKey` with refresh family and index helpers.
17. Add or update refresh record structs in `store/records.rs`.
18. Implement `store/refresh.rs` typed operations.
19. Update authorization-code exchange to create refresh tokens when `offline_access` is granted.
20. Implement target `grant_type=refresh_token` in `/token` using typed store rotation.
21. Implement `POST /oauth/revoke`.
22. Mount `POST /oauth/revoke` in `routes.rs`.
23. Update discovery metadata only after the route exists and tests pass.
24. Emit sanitized audit events.
25. Remove target route dependencies on legacy refresh JWT signing and raw-token storage.
26. Run focused Rust tests for `refresh_logout_slice` and `token_exchange_slice`.
27. Run full Rust tests.
28. Run `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Store Tests

- `create_refresh_token` returns a raw refresh token only to the caller.
- Primary refresh record is keyed by HMAC digest, not raw refresh token.
- Family record is keyed by generated family ID, not raw refresh token.
- Subject index record contains no raw refresh token.
- Client index record contains no raw refresh token.
- All refresh records carry `expires_at`.
- All refresh records use DynamoDB TTL equal to `expires_at`.
- Refresh token TTL uses `AUTH_REFRESH_TOKEN_TTL_SECONDS`.
- `rotate_refresh_token` creates a new raw token and new digest.
- `rotate_refresh_token` marks the old record with `replaced_by`.
- `rotate_refresh_token` preserves family ID, subject, client ID, scope, subject type, and properties.
- `rotate_refresh_token` is atomic when the old record changes concurrently.
- Reusing a replaced token revokes the family.
- Reusing a revoked token leaves the family revoked and returns a reuse outcome.
- Revoking an existing token family marks the family revoked.
- Revoking a missing token returns an idempotent success outcome.
- Revoking a wrong-client token returns an idempotent success outcome without revoking the other client's family.

### Authorization-Code Exchange Tests

- `offline_access` authorization-code exchange returns `refresh_token`.
- Returned refresh token is not a JWT.
- Returned refresh token is not stored in DynamoDB keys.
- Exchange without `offline_access` omits `refresh_token`.
- Exchange with `offline_access` requires the client to allow `refresh_token` grant.
- Exchange with `offline_access` keeps issuing access tokens and ID tokens as slice 05 did.
- Exchange with `offline_access` emits sanitized `refresh_token_issued` audit event.

### Refresh Grant Tests

- Valid refresh token returns a new access token and new refresh token.
- Refresh response does not return an ID token in this slice.
- New access token validates with runtime signer, issuer, and `AUTH_ACCESS_TOKEN_AUDIENCE`.
- New access token preserves subject, subject type, scope, and properties from the refresh record.
- Old refresh token cannot be used again.
- New refresh token can be used for the next rotation.
- Missing refresh token returns `invalid_grant`.
- Expired refresh token returns `invalid_grant`.
- Refresh token for another client returns `invalid_grant`.
- Deleted account cannot refresh.
- Disabled account fails if disabled status exists by implementation time.
- Raw refresh token does not appear in route errors or audit logs.

### Reuse Detection Tests

- Submitting a replaced refresh token fails.
- Submitting a replaced refresh token revokes the family.
- After reuse is detected, the replacement token from the same family also fails.
- Reuse detection emits sanitized `refresh_token_reuse_detected`.
- Concurrent rotation conflict is handled as reuse if reload shows replacement or revocation.

### Revoke Route Tests

- `POST /oauth/revoke` with the current refresh token returns HTTP 200.
- Refresh after revoke fails.
- Revoke of the same token twice returns HTTP 200 both times.
- Revoke of a random/missing token returns HTTP 200 after valid client authentication.
- Public client cannot revoke another client's refresh token.
- Confidential client must authenticate before revocation.
- Invalid confidential client auth returns `invalid_client`.
- Revoke response does not reveal whether the token existed.
- Revoke does not revoke already-issued access tokens.

### Discovery Tests

- OpenID metadata advertises `authorization_code` and `refresh_token`.
- OpenID metadata includes `offline_access`.
- OpenID metadata includes `revocation_endpoint`.
- OAuth metadata matches the implemented grant and endpoint set.
- Metadata does not advertise `introspection_endpoint`.
- Metadata does not advertise `client_credentials`.

### Audit And Secret Handling Tests

- `refresh_token_issued` contains no raw refresh token.
- `refresh_token_rotated` contains no old or new raw refresh token.
- `refresh_token_reuse_detected` contains no raw refresh token.
- `refresh_family_revoked` contains no raw refresh token.
- `user_logout_refresh_token_revoked` contains no raw refresh token.
- Token references, if present, are digest-only.

## Acceptance Criteria

- Authorization-code exchange with `offline_access` returns a refresh token.
- Authorization-code exchange without `offline_access` does not return a refresh token.
- Refresh tokens are high-entropy opaque bearer values, not JWTs.
- Refresh tokens are stored and looked up by HMAC digest.
- Raw refresh tokens never appear in DynamoDB keys, logs, audit events, or errors.
- Refresh-token records, family records, and index records all carry TTL.
- Refresh-token rotation is atomic.
- Every successful refresh rotates the refresh token.
- Reuse of a replaced or revoked refresh token is detected.
- Reuse detection revokes the refresh family.
- New token issuance from refresh requires an active account.
- Refresh grant issues runtime-signed access tokens.
- Refresh grant does not issue ID tokens in this slice.
- User-facing `POST /oauth/revoke` revokes the submitted refresh token family.
- Revocation is idempotent and does not reveal token existence.
- A client cannot revoke another client's refresh token.
- Revocation does not invalidate already-issued access JWTs.
- Discovery advertises `refresh_token`, `offline_access`, and `revocation_endpoint` only after implementation.
- Discovery still does not advertise introspection, opaque access tokens, or `client_credentials`.
- Target refresh and revoke routes do not use legacy raw-token storage or table-wide scans.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test refresh_logout_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml --test token_exchange_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

Manual protocol smoke test after implementation:

```text
GET /authorize?response_type=code&client_id=web&redirect_uri=<registered>&state=abc&scope=openid%20email%20offline_access&provider=password&code_challenge=<challenge>&code_challenge_method=S256&nonce=n-123
POST /password/login
POST /token grant_type=authorization_code client_id=web code=<code> redirect_uri=<registered> code_verifier=<verifier>
POST /token grant_type=refresh_token client_id=web refresh_token=<refresh_token>
POST /oauth/revoke client_id=web token=<latest_refresh_token> token_type_hint=refresh_token
POST /token grant_type=refresh_token client_id=web refresh_token=<latest_refresh_token>
GET /.well-known/openid-configuration
```

Expected result:

- Code exchange returns access token, ID token, and refresh token.
- Refresh grant returns a new access token and a new refresh token.
- Reusing an old refresh token fails.
- Revoke returns success.
- Refresh after revoke fails.
- Discovery advertises refresh and revoke.
- DynamoDB contains no raw refresh token in `pk` or `sk`.
- Access tokens issued before revoke remain valid until `exp`.

AWS validation can wait until the AWS hardening slice. This slice can be validated locally because it does not depend on API Gateway source-IP behavior or KMS.

## Next Slice

After this slice, implement `07_google_and_apple_oidc_login`.

That slice should:

- add Google OIDC start and callback flow
- add Apple OIDC start and callback flow
- store provider state by HMAC digest
- validate provider issuer, audience, nonce, expiry, and signature
- map issuer plus provider subject to internal persisted identities
- avoid automatic account linking by matching email
