# 18_legacy_storage_and_security_regression

## Goal

Remove the remaining compiled legacy paths that conflict with the target auth design, then add static and runtime regression checks for the security properties the rewrite is meant to protect.

At the end of this slice, the auth crate should no longer compile the old custom-admin API, runtime OAuth client CRUD, DynamoDB signing-key storage, or raw-refresh-token rotation helpers. Target runtime paths should use config-only clients, runtime signing, typed refresh storage, typed one-time secret storage, and the separate IAM-protected account lifecycle API.

## Design Docs Followed

This slice should follow these design documents:

- `design/migration.md`
- `design/scope.md`
- `design/auth/api/admin.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/config/client-file.md`
- `design/auth/config/clients.md`
- `design/auth/core/clients.md`
- `design/auth/core/tokens.md`
- `design/auth/crypto/signing.md`
- `design/auth/store/dynamodb.md`
- `design/auth/store/keys.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/testing.md`
- `design/auth/threat-model.md`

The important design constraints are:

- OAuth clients are config-only in v1.
- Account lifecycle admin routes are in the separate IAM-protected admin Lambda.
- No public or custom-key runtime admin control plane exists in the auth Lambda.
- Token signing uses the configured runtime signer, not plaintext private keys in `AuthTable`.
- Refresh tokens are stored through typed HMAC-keyed refresh records.
- Runtime auth paths do not store raw bearer values in DynamoDB keys.
- Runtime auth paths do not scan raw auth-state tables for normal protocol behavior.

## Why This Slice Next

Slice 17 removed the legacy provider/UI surface. The remaining target mismatch is mostly compiled but unmounted code:

```text
packages/functions/auth/src/admin/*
packages/functions/auth/src/client/registry.rs
packages/functions/auth/src/jwt/keys.rs
legacy refresh helpers in packages/functions/auth/src/oauth/token.rs
```

Those paths preserve old behavior the target design intentionally rejects:

```text
custom admin API keys
public bootstrap helper code
runtime OAuth client CRUD
plaintext JWT private keys in DynamoDB
raw refresh tokens as DynamoDB keys
refresh revocation by table scan
```

This slice removes or quarantines those paths before the next AWS deployment validation pass.

## In Scope

### Legacy Admin Module Removal

Remove the old custom-key admin module tree:

```text
packages/functions/auth/src/admin/auth.rs
packages/functions/auth/src/admin/clients.rs
packages/functions/auth/src/admin/tokens.rs
packages/functions/auth/src/admin/mod.rs
```

Remove module exports:

```text
mod admin;
pub mod admin;
```

from:

```text
packages/functions/auth/src/main.rs
packages/functions/auth/src/lib.rs
```

Keep the target IAM admin API:

```text
packages/functions/auth/src/api/admin.rs
packages/functions/admin/src/main.rs
```

The old `bootstrap`, `AdminKey`, `authenticate_admin_key`, client-management handlers, and token revocation handlers should no longer compile into the auth crate.

### Runtime Client CRUD Removal

Remove the old DynamoDB-backed client registry and request/response shapes used only by runtime client CRUD:

```text
packages/functions/auth/src/client/registry.rs
```

Then collapse `packages/functions/auth/src/client` to the minimal helper surface still used by target routes:

```text
parse_basic_auth
```

Acceptable code shape:

```text
packages/functions/auth/src/client/mod.rs
```

owns `parse_basic_auth` directly, and legacy `Client`, `CreateClientRequest`, `UpdateClientRequest`, `validate_authorize_request`, and `validate_token_request` are removed if no target path uses them.

Do not remove the target config-only client model:

```text
packages/functions/auth/src/core/clients.rs
packages/functions/auth/src/config/client_file.rs
```

### Legacy JWT Signing-Key Storage Removal

Remove the old DynamoDB signing-key helper path:

```text
packages/functions/auth/src/jwt/keys.rs
```

Remove exports from:

```text
packages/functions/auth/src/jwt/mod.rs
```

Target signing remains in:

```text
packages/functions/auth/src/crypto/signing.rs
packages/functions/auth/src/crypto/kms_signing.rs
```

If `LocalEs256Signer::generate()` still depends on `generate_signing_key`, move that generation helper into `crypto/signing.rs` so local tests and local mode do not depend on `jwt/keys.rs`.

After this cleanup, no production token path should reference:

```text
signing:key
private_key_pem in AuthTable
get_or_create_signing_key
get_all_signing_keys
```

### Legacy Refresh Helper Removal

Remove the old refresh implementation from:

```text
packages/functions/auth/src/oauth/token.rs
```

Remove:

```text
RefreshTokenRecord
handle_refresh_token_grant
rotate_refresh_record
log_refresh_event
revoke_refresh_tokens
hash_token
```

and the imports used only by those helpers:

```text
sha2::{Digest, Sha256}
crate::jwt::keys::get_or_create_signing_key
crate::jwt::sign::{sign_access_token, sign_refresh_token}
crate::jwt::verify::verify_refresh_token
crate::storage::{TransactCondition, TransactOperation}
```

Keep the target refresh path:

```text
handle_target_refresh_token_grant
AuthStore::create_refresh_token
AuthStore::rotate_refresh_token
AuthStore::revoke_refresh_token_family
AuthStore::revoke_refresh_tokens_for_subject
```

Old unit tests that directly exercise raw-token `oauth:refresh/<raw-token>` records should be deleted or rewritten against the typed refresh store tests in `refresh_logout_slice.rs`.

### Static Security Regression Validation

Add or extend a validator script:

```text
scripts/validate-legacy-removal.mjs
```

or create:

```text
scripts/validate-auth-security-regression.mjs
```

and call it from:

```text
npm run test:infra
```

Required checks:

- `packages/functions/auth/src/admin` does not exist.
- `packages/functions/auth/src/client/registry.rs` does not exist.
- `packages/functions/auth/src/jwt/keys.rs` does not exist.
- `main.rs` and `lib.rs` do not export the legacy `admin` module.
- `jwt/mod.rs` does not export `keys`.
- `oauth/token.rs` does not contain `handle_refresh_token_grant`.
- `oauth/token.rs` does not contain `rotate_refresh_record`.
- `oauth/token.rs` does not contain `revoke_refresh_tokens`.
- `oauth/token.rs` does not contain raw `["oauth:refresh", refresh_token_str]` lookups.
- Runtime source files do not contain `["signing:key"` writes or scans.
- Runtime source files do not contain `["admin:key"` scans.
- Runtime source files do not contain `scan_page(&["client"]`.
- Public routes do not mount client-management, token-introspection, bootstrap, or custom-key admin endpoints.

The validator may allow `scan` in typed store modules where the access pattern is a bounded partition query represented through the current storage adapter. Do not block existing test-only scans in `packages/functions/auth/tests`.

### Runtime Regression Tests

Keep or add focused tests that prove:

- Public `/admin/bootstrap` remains `404 Not Found`.
- Public runtime client-management routes remain `404 Not Found`.
- `/token` rejects `client_credentials`.
- `/oauth/revoke` uses typed refresh-token family revocation and remains idempotent.
- Refresh-token reuse still revokes the family.
- JWKS still comes from the runtime signer and no `signing:key` records are written.
- Authorization-code exchange with `offline_access` stores refresh tokens by HMAC digest.
- A raw refresh token cannot be found at `["oauth:refresh", <raw token>]`.
- The target IAM admin router still rejects requests without IAM authorizer context.

Prefer strengthening existing tests over adding broad duplicate suites.

## Out Of Scope

- Removing `StorageAdapter` entirely.
- Removing test-only in-process storage helpers.
- Rewriting the DynamoDB adapter into `packages/functions/auth/src/store/dynamo.rs`.
- Replacing every typed-store internal scan with DynamoDB Query in this slice.
- AWS live deployment testing.
- New auth flows.
- Generic OIDC providers.
- Hosted UI.
- Payments or dashboard work.

## Expected Code Shape

Likely deletions:

```text
packages/functions/auth/src/admin/
packages/functions/auth/src/client/registry.rs
packages/functions/auth/src/jwt/keys.rs
```

Likely Rust modifications:

```text
packages/functions/auth/src/client/mod.rs
packages/functions/auth/src/client/validation.rs
packages/functions/auth/src/client/types.rs
packages/functions/auth/src/crypto/signing.rs
packages/functions/auth/src/jwt/mod.rs
packages/functions/auth/src/lib.rs
packages/functions/auth/src/main.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/tests/runtime_route_slice.rs
packages/functions/auth/tests/token_exchange_slice.rs
packages/functions/auth/tests/refresh_logout_slice.rs
```

Likely script modifications:

```text
scripts/validate-legacy-removal.mjs
package.json
```

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add static validation failures for the remaining legacy admin/client/signing/refresh symbols.
2. Run `npm run test:infra` and confirm the new validation fails for the expected legacy paths.
3. Add or tighten route/security tests for public bootstrap, public client management, client credentials rejection, raw refresh-token key absence, and runtime signer JWKS.
4. Remove `mod admin` and `pub mod admin`.
5. Delete `packages/functions/auth/src/admin/`.
6. Move `parse_basic_auth` into the minimal target client module surface.
7. Delete the old DynamoDB client registry and legacy client request/response types.
8. Move local ES256 key generation out of `jwt/keys.rs` if still needed.
9. Delete `jwt/keys.rs` and remove `pub use keys::*`.
10. Remove legacy refresh helpers from `oauth/token.rs`.
11. Delete or rewrite old unit tests that exercised raw refresh-token records.
12. Run focused tests for token exchange, refresh/logout, runtime routes, and KMS signing.
13. Run full Rust tests, `cargo check`, `npm run typecheck`, `npm run test:infra`, `npm run test:setup`, and `git diff --check`.

## Tests

### Static Validation

- `src/admin` is absent.
- `src/client/registry.rs` is absent.
- `src/jwt/keys.rs` is absent.
- No old custom admin API key symbols remain in runtime source.
- No runtime client CRUD symbols remain in runtime source.
- No `signing:key` table helper remains in runtime source.
- No legacy refresh helper remains in `oauth/token.rs`.
- No raw refresh token lookup remains in `oauth/token.rs`.

### Route And Protocol Tests

- `POST /admin/bootstrap` returns `404 Not Found`.
- `POST /admin/clients` returns `404 Not Found`.
- `POST /token` with `grant_type=client_credentials` returns `unsupported_grant_type`.
- `POST /oauth/revoke` returns success for a valid refresh token and stays idempotent.
- Refresh token reuse returns `invalid_grant` and revokes the refresh family.
- JWKS returns the runtime signer public key.
- Token exchange does not write `signing:key` records.
- Refresh-token storage cannot be looked up by raw refresh token.

### Regression Tests To Keep Green

- Password registration and verification do not issue tokens.
- Password login issues only an authorization code.
- Authorization-code exchange validates PKCE.
- Userinfo rejects ID tokens.
- Google and Apple callback flows still issue internal authorization codes.
- Account disable/delete routes remain IAM-protected in the separate admin Lambda.
- KMS ES256 mode signs JWTs and publishes public JWKS material.

## Acceptance Criteria

- Old custom-key admin code no longer compiles into the auth crate.
- Runtime OAuth client CRUD code is removed.
- Config-only clients remain the only client source for target OAuth routes.
- JWT signing-key private material is not read from or written to DynamoDB.
- Legacy raw-refresh-token storage and scan helpers are removed.
- Target refresh-token rotation and logout tests still pass.
- Static validation prevents the removed legacy paths from returning.
- The auth crate still supports local ES256 and KMS ES256 runtime signing.
- The public auth Lambda remains API-only.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:infra
npm run test:setup
git diff --check
```

Focused validation during implementation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test runtime_route_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml --test token_exchange_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml --test refresh_logout_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml --test kms_signing_slice
```

AWS validation is not required for this slice. This slice removes compiled legacy paths and tightens local/static regression checks. AWS dev validation should happen after this cleanup when the repo is ready for an end-to-end deployment smoke test.

## Next Slice

After this slice, define a deployment validation slice.

That slice should deploy to the AWS dev account and verify:

- API Gateway source IP rate-limit identity.
- IAM protection on `/_admin/*`.
- DynamoDB `pk` and `sk` shapes contain no raw bearer values.
- DynamoDB TTL attributes are present on short-lived auth records.
- KMS/table/signing configuration matches the selected stage settings.
- CloudWatch audit logging mode and retention match config.
