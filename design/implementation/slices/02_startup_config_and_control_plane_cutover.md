# 02_startup_config_and_control_plane_cutover

## Goal

Wire the slice 01 foundation into the running Lambda and remove the old runtime control plane from the target router.

At the end of this slice, deployed auth should use config-only OAuth clients loaded at startup, and the public router should no longer expose first-deployer-wins bootstrap or runtime client-management routes.

## Why This Slice Next

Slice 01 added the safe primitives but left the legacy runtime paths in place. The next useful step is to make the Lambda actually depend on the new configuration model before adding password login.

This keeps the scope narrow and directly addresses the highest-risk leftover behavior:

- public `/admin/bootstrap`
- runtime OAuth client creation/update/deletion
- DynamoDB-backed OAuth client lookup
- metadata that must not drift from implemented behavior

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/config/environment.md`
- `design/auth/config/client-file.md`
- `design/auth/config/clients.md`
- `design/auth/config/ttls.md`
- `design/auth/config/account-lifecycle.md`
- `design/auth/config/stages.md`
- `design/auth/core/clients.md`
- `design/auth/core/scopes.md`
- `design/auth/crypto/hmac-lookups.md`
- `design/auth/crypto/signing.md`
- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/api/admin.md`
- `design/auth/store/keys.md`
- `design/infra/secrets.md`

## In Scope

### Startup Config Loading

Add startup loading for:

- `AUTH_CLIENT_CONFIG_PATH`, default `auth.clients.toml`
- config-only OAuth clients
- confidential-client secret refs from local environment or SST-provided environment
- HMAC lookup secret setting, failing startup if missing
- typed TTL config
- deleted identity reuse config
- audit log mode config
- signing mode config for `local-es256`

Rules:

- Startup fails before serving requests when required config is missing or invalid.
- Validation errors name the setting or client but do not print secret values.
- Client secret refs are names only. The resolved secret value is hashed in memory and not stored in DynamoDB.
- `AUTH_CLIENT_CONFIG_PATH` must resolve inside the packaged Lambda working tree or another explicitly supported path.

### Application State

Extend application state with the new foundation objects:

- read-only client registry
- HMAC lookup secret material or lookup service
- TTL config
- account lifecycle config
- audit log mode
- signer abstraction

The target state should make it hard for new route code to reach legacy client storage accidentally.

### Config-Only Client Cutover

Replace DynamoDB-backed client validation in target auth paths with the config registry.

Required behavior:

- `client_id` lookup is exact.
- redirect URI matching is exact.
- public clients require PKCE for authorization code flow.
- confidential clients verify against the resolved secret hash.
- `client_credentials` stays unsupported.
- unsupported grant types fail before token issuance.

This slice may keep token issuance behavior legacy where unavoidable, but client lookup and validation should no longer depend on DynamoDB client records.

### Control-Plane Route Removal

Remove or disable target router exposure for:

- `POST /admin/bootstrap`
- `/admin/clients`
- client secret rotation
- runtime client delete/update/list routes

Operator account lifecycle routes are not introduced in this slice. They come later under IAM-protected `/_admin/*`.

## Out Of Scope

- Password registration, verification, login, or reset.
- Resend email delivery.
- Google or Apple login.
- Refresh-token storage rewrite.
- Account deletion routes.
- KMS signing implementation.
- DynamoDB customer managed KMS.
- Full removal of legacy source files that are no longer routed.

## Expected Code Shape

Target modules:

```text
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/config/client_file.rs
packages/functions/auth/src/core/clients.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/oauth/authorize.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/main.rs
```

Existing modules may keep old code temporarily if it is no longer reachable from the target router.

## Detailed Work Plan

1. Add a `RuntimeAuthConfig` loader that reads env values and `auth.clients.toml`.
2. Add tests for missing client file, malformed client file, missing HMAC secret, and missing confidential-client secret.
3. Add a read-only client registry wrapper around validated `ConfiguredClient` values.
4. Add exact client lookup and secret verification tests.
5. Extend `AppState` with the runtime config objects.
6. Wire `main.rs` startup to load runtime config before router creation.
7. Update authorize request validation to use the config registry.
8. Update token request client authentication to use the config registry.
9. Reject `client_credentials` in token handling even if older code remains compiled.
10. Remove `/admin/bootstrap` from the router.
11. Remove or stop mounting `/admin/clients` routes from the target router.
12. Add HTTP/router tests proving removed routes are not exposed.
13. Run full Rust tests and TypeScript typecheck.

## Tests

### Startup Config Tests

- Missing `auth.clients.toml` path fails.
- Malformed client config fails.
- Public client with `client_secret_ref` fails.
- Confidential client missing secret ref fails.
- Confidential client with missing resolved secret fails.
- Missing HMAC lookup secret fails.
- Invalid TTL relationships fail.
- Invalid deleted identity reuse mode fails.
- Invalid signing config fails.

### Client Registry Tests

- Exact `client_id` lookup succeeds.
- Unknown `client_id` fails.
- Exact redirect URI match succeeds.
- Redirect URI with changed host/path/query fails.
- Public clients require PKCE.
- Confidential client secret verification succeeds for the correct secret.
- Confidential client secret verification fails for the wrong secret.
- `client_credentials` is rejected.

### Route Tests

- `/.well-known/openid-configuration` still succeeds.
- `POST /admin/bootstrap` returns not found or method not allowed.
- `/admin/clients` routes are not mounted.
- `/authorize` rejects unknown clients through config registry behavior.
- `/token` rejects unsupported grant types before issuing tokens.

## Acceptance Criteria

- Lambda startup uses validated auth runtime config.
- OAuth clients come from `auth.clients.toml`, not DynamoDB.
- No public bootstrap route is exposed by the target router.
- Runtime client create/update/delete/list routes are not exposed by the target router.
- Client validation has focused tests that do not require DynamoDB records.
- Metadata remains aligned with implemented behavior.
- No password or provider login work is introduced.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
```

AWS validation after deploy:

```text
curl <api-url>/.well-known/openid-configuration
curl -i -X POST <api-url>/admin/bootstrap
```

Expected AWS result:

- discovery returns configured issuer metadata
- `/admin/bootstrap` is not available

## Next Slice

After this slice, implement `03_password_registration_and_email_verification`.

That slice should add the first password account flow on top of the new config and store foundation:

- password registration
- verification secret creation
- Resend email delivery
- verification consume
- verified password identity creation
- no login or authorization-code issuance yet
