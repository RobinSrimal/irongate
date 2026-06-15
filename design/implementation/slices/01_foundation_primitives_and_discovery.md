# 01_foundation_primitives_and_discovery

## Goal

Create the first secure foundation layer for the auth service before cutting over user login or token routes.

At the end of this slice, the codebase has tested primitives for config-only clients, HMAC lookups, typed store keys, account/identity records, generated subjects, local ES256 JWKS output, and public discovery metadata.

## Why This Slice First

Most security issues in the current code come from generic storage, runtime control-plane behavior, and unclear secret handling. This slice removes that ambiguity before adding password or provider flows on top.

It is meaningful even without login because it produces observable behavior:

- `/.well-known/openid-configuration`
- `/.well-known/oauth-authorization-server`
- `/.well-known/jwks.json`
- parse/validation failures for bad client and runtime config primitives
- tested typed store/key behavior

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/config/environment.md`
- `design/auth/config/clients.md`
- `design/auth/config/client-file.md`
- `design/auth/config/ttls.md`
- `design/auth/config/account-lifecycle.md`
- `design/auth/core/clients.md`
- `design/auth/core/scopes.md`
- `design/auth/core/subjects.md`
- `design/auth/core/identities.md`
- `design/auth/core/account-lifecycle.md`
- `design/auth/crypto/hmac-lookups.md`
- `design/auth/crypto/signing.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/store/keys.md`
- `design/auth/store/records.md`
- `design/auth/store/accounts.md`
- `design/auth/store/identities.md`

## In Scope

### Configuration

Implement typed configuration primitives for:

- config-only OAuth clients
- token and short-lived artifact TTLs
- deleted identity reuse policy
- audit log mode
- signing mode

Required design inputs:

- `design/auth/config/environment.md`
- `design/auth/config/clients.md`
- `design/auth/config/client-file.md`
- `design/auth/config/ttls.md`
- `design/auth/config/account-lifecycle.md`

### Client Registry

Implement static client config parsing and validation for `auth.clients.toml`.

Rules:

- Validate client IDs, redirect URIs, grant types, scopes, PKCE rules, and token endpoint auth method.
- Provide a resolver hook for confidential client secret refs.
- Store only derived hashes for secret verification inside runtime memory.
- Do not read or write OAuth clients from DynamoDB.
- Do not expose runtime client create/update/delete routes.

### Store Foundation

Implement the typed auth store facade and key helpers over the current storage adapter.

Required store families for this slice:

- account records
- identity records
- authorize session records
- provider state records
- authorization code records
- password verification/reset secret records
- refresh token records
- rate-limit records, if needed by route middleware skeletons

This slice does not need every full flow that uses those records. It should establish key construction and the first typed account/identity operations where later slices depend on them.

Rules:

- Raw bearer values never appear in `pk`, `sk`, logs, or errors.
- HMAC lookup digests are used for tokens, codes, state, sessions, email lookup, and provider identities.
- Runtime expiry checks reject expired records before DynamoDB TTL deletion.
- Store methods expose purpose-specific operations, not generic route-level `set/get/remove`.

### Account And Identity Foundation

Implement:

- generated opaque subject IDs
- account records keyed by subject
- password, Google, and Apple identity digest helpers
- identity creation from verified proof
- identity tombstone/reuse helpers
- active-account checks

Rules:

- `sub` is generated and persisted.
- `sub` is never re-derived from email or provider claims.
- Deleted subjects are never reused.
- Deleted identity reuse may create a new subject according to config.

### Signing, Discovery, And JWKS

Implement enough signing infrastructure for discovery and JWKS tests.

Target for this slice:

- local ES256 signing key loading
- public JWKS generation
- signer interface that can later support KMS ES256
- OIDC discovery metadata
- OAuth authorization-server metadata

Metadata must advertise only implemented behavior:

- authorization code flow
- refresh token grant
- revocation endpoint if route wiring exists in skeleton form
- ES256
- supported scopes
- no token introspection
- no `client_credentials`

## Out Of Scope

- Loading `auth.clients.toml` into Lambda startup state.
- Cutting `/authorize` or `/token` over to config-only clients.
- Removing old admin/client routes from the router.
- Password registration, verification, or login.
- Resend email delivery.
- Token issuance from `/token`.
- Refresh-token rotation.
- `/oauth/revoke` behavior.
- `/userinfo` behavior.
- Google or Apple callback flows.
- IAM-protected admin routes.
- KMS ES256 signing implementation.
- Customer managed DynamoDB KMS key implementation.
- Hosted UI or app/reference website work.

## Expected Code Shape

Target modules:

```text
packages/functions/auth/src/config/
packages/functions/auth/src/core/subjects.rs
packages/functions/auth/src/core/clients.rs
packages/functions/auth/src/core/scopes.rs
packages/functions/auth/src/store/
packages/functions/auth/src/crypto/hmac_lookup.rs
packages/functions/auth/src/crypto/signing.rs
packages/functions/auth/src/oauth/well_known.rs
```

SST/infra changes should stay narrow:

```text
infra/api.ts
infra/storage.ts
sst.config.ts
```

## Detailed Work Plan

1. Add typed config structs and pure validation.
2. Add `auth.clients.toml` example with non-secret client settings.
3. Add static client config parser and exact client lookup.
4. Add HMAC lookup helper using a server-side lookup secret.
5. Add typed DynamoDB key constructors.
6. Add typed record structs with explicit `created_at` and `expires_at` where needed.
7. Add typed account/identity store operations needed by future flows.
8. Add generated subject/account primitives.
9. Add identity digest and identity record primitives.
10. Add local ES256 signer and JWKS public key output.
11. Add discovery endpoints.
12. Add focused tests.

## Tests

### Config Tests

- Malformed client config fails validation.
- Invalid redirect URI fails validation.
- Public client with secret ref fails validation.
- Confidential client missing secret ref fails validation.
- Invalid TTL values fail validation.
- Unknown deleted identity reuse mode fails validation.
- `local-es256` without signing key material fails validation.

### Store And Crypto Tests

- HMAC lookup output is deterministic for the same input.
- Different key families produce different lookup digests for the same raw value.
- Raw authorization code, refresh token, provider state, session key, and email do not appear in store keys.
- Expired records are rejected before DynamoDB TTL deletion.
- Account creation generates an opaque subject.
- Identity creation maps verified proof to a generated subject.
- Deleted identity reuse creates a new subject and never reuses the old one.

### Discovery Tests

- OIDC metadata uses configured issuer.
- Metadata advertises authorization code and refresh token grants.
- Metadata does not advertise introspection.
- Metadata does not advertise `client_credentials`.
- JWKS exposes public key material only.
- JWKS key ID matches signer metadata.

## Acceptance Criteria

- Client and runtime config primitives reject invalid settings in tests.
- Static OAuth client definitions are parsed and validated from `auth.clients.toml` shape.
- Store foundation tests prove raw bearer values are absent from typed keys.
- Account and identity store operations use generated opaque subjects.
- Raw bearer values are absent from `pk` and `sk` in tests.
- Discovery and JWKS endpoints are implemented and tested.
- No hosted UI or app behavior is introduced.
- Existing unsafe admin bootstrap is not used by any new code path.

## Manual Validation

For local validation:

```text
cargo test
cargo check
```

For AWS validation after this slice:

```text
sst deploy --stage dev
curl <api-url>/.well-known/openid-configuration
curl <api-url>/.well-known/jwks.json
```

The AWS validation should confirm:

- API Gateway routes reach the Rust Lambda.
- Discovery issuer equals the configured public issuer.
- JWKS returns public key material and no private key data.

## Next Slice

After this slice, implement `02_startup_config_and_control_plane_cutover`.

That slice wires the foundation into the Lambda runtime:

- load `auth.clients.toml` at startup
- resolve secret refs
- put the read-only client registry into app state
- make authorize/token validation use config clients
- remove the public bootstrap and runtime client-management routes from the target router

## Risks

- Trying to implement password login inside this slice would make it too large.
- KMS signing can distract from the local signer/JWKS foundation; keep the interface now and KMS implementation later.
- Accidentally treating this foundation as a full route cutover would hide remaining legacy runtime risk.
- Client secret refs must not be confused with plaintext client secrets in checked-in config.
