# 01_foundation_config_store_and_discovery

## Goal

Create the secure runtime foundation for the auth service before implementing user login.

At the end of this slice, the Lambda should be able to start with validated configuration, load config-only OAuth clients, expose discovery/JWKS metadata, and use typed DynamoDB store primitives for accounts, identities, and short-lived records.

## Why This Slice First

Most security issues in the current code come from generic storage, runtime control-plane behavior, and unclear secret handling. This slice removes that ambiguity before adding password or provider flows on top.

It is meaningful even without login because it produces observable behavior:

- `/.well-known/openid-configuration`
- `/.well-known/oauth-authorization-server`
- `/.well-known/jwks.json`
- startup validation failures for bad config
- tested typed store/key behavior

## In Scope

### Configuration

Implement typed runtime configuration for:

- issuer URL
- client config file path
- config-only OAuth clients
- HMAC lookup secret reference/value
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

Implement a read-only client registry loaded at startup from `auth.clients.toml`.

Rules:

- Validate client IDs, redirect URIs, grant types, scopes, PKCE rules, and token endpoint auth method.
- Resolve confidential client secret refs from deployed secrets or local environment variables.
- Store only derived hashes for secret verification inside runtime memory.
- Do not read or write OAuth clients from DynamoDB.
- Do not expose runtime client create/update/delete routes.

### Store Foundation

Implement the concrete DynamoDB auth store facade and typed key helpers.

Required store families for this slice:

- account records
- identity records
- authorize session records
- provider state records
- authorization code records
- password verification/reset secret records
- refresh token records
- rate-limit records, if needed by route middleware skeletons

This slice does not need every full flow that uses those records, but the typed operations and key construction should exist where later slices depend on them.

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

Implement enough signing infrastructure for discovery/JWKS.

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
packages/functions/auth/src/crypto/hmac.rs
packages/functions/auth/src/crypto/signing.rs
packages/functions/auth/src/api/oauth/discovery.rs
```

SST/infra changes should stay narrow:

```text
infra/api.ts
infra/storage.ts
sst.config.ts
```

## Detailed Work Plan

1. Add typed config structs and startup validation.
2. Add `auth.clients.toml` example with non-secret client settings.
3. Add config-only client registry and exact client lookup.
4. Add HMAC lookup helper using a server-side lookup secret.
5. Add typed DynamoDB key constructors.
6. Add typed record structs with explicit `created_at` and `expires_at` where needed.
7. Add DynamoDB store operations needed by future flows.
8. Add generated subject/account primitives.
9. Add identity digest and identity record primitives.
10. Add local ES256 signer and JWKS public key output.
11. Add discovery endpoints.
12. Remove or bypass old discovery/client/storage paths that conflict with this slice.
13. Add focused tests.

## Tests

### Config Tests

- Missing client config file fails startup.
- Malformed client config fails startup.
- Invalid redirect URI fails startup.
- Public client with secret ref fails startup.
- Confidential client missing secret ref fails startup.
- Missing HMAC lookup secret fails startup.
- Invalid TTL values fail startup.
- Unknown deleted identity reuse mode fails startup.
- `local-es256` without signing key material fails startup.

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

- Lambda startup fails fast for invalid auth configuration.
- OAuth clients are loaded from config, not DynamoDB.
- No runtime route can create or mutate OAuth clients.
- DynamoDB store access from new code goes through typed store methods.
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

## Risks

- Trying to implement password login inside this slice would make it too large.
- KMS signing can distract from the local signer/JWKS foundation; keep the interface now and KMS implementation later.
- Leaving generic storage reachable from new code would undermine the rest of the rewrite.
- Client secret refs must not be confused with plaintext client secrets in checked-in config.
