# 17_legacy_provider_ui_route_removal

## Goal

Remove the legacy dynamic provider and hosted-UI route surface from the public auth Lambda.

At the end of this slice, the runtime should expose only the target API-only routes for OAuth/OIDC, password auth, Google OIDC, Apple OIDC, user-facing refresh-token revocation, and IAM-protected account lifecycle. The legacy `packages/functions/auth/src/provider` tree, built-in HTML auth UI modules, dynamic `/:provider/*` routes, and generic provider deployment config should no longer compile into the auth Lambda.

This slice intentionally does not remove the generic storage trait, local in-process test storage, unmounted legacy admin/client-management modules, or old DynamoDB signing-key helpers. Those remain for the next cleanup/security-regression slice.

## Design Docs Followed

This slice should follow these design documents:

- `design/scope.md`
- `design/migration.md`
- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/api/providers/password.md`
- `design/auth/api/providers/google.md`
- `design/auth/api/providers/apple.md`
- `design/auth/providers/README.md`
- `design/auth/providers/password.md`
- `design/auth/providers/google.md`
- `design/auth/providers/apple.md`
- `design/auth/testing.md`
- `design/auth/threat-model.md`

The important design constraints are:

- The auth foundation is API-only.
- The target providers are password, Google, and Apple.
- Generic arbitrary OAuth2 identity providers are out of v1.
- Email OTP or magic-link login is out of v1.
- Built-in login, registration, reset, provider-selection, or consent UI is out of v1.
- OAuth clients remain config-only; runtime provider/client management does not return through this cleanup.

## Why This Slice Next

After KMS signing, the largest mismatch between the running code and the target design is the legacy provider surface:

```text
/:provider/authorize
/:provider/callback
src/provider/*
src/ui/*
PROVIDERS / PROVIDER_*
```

Those paths exist for the original hosted UI, generic OAuth2/OIDC dispatch, passwordless code provider, and runtime provider config model. The target core has already replaced them with explicit API-only routes:

```text
/authorize
/token
/userinfo
/oauth/revoke
/password/*
/google/*
/apple/*
/_admin/*
```

Removing the legacy provider/UI surface first makes the later security-regression slice smaller and gives a clear answer that `src/provider` is no longer needed.

## In Scope

### Public Router Surface Cleanup

Update the public auth router so it mounts only target API routes.

Allowed public auth routes:

```text
GET /.well-known/oauth-authorization-server
GET /.well-known/openid-configuration
GET /.well-known/jwks.json
GET /authorize
POST /token
GET /userinfo
POST /oauth/revoke
POST /password/register
POST /password/verify
POST /password/login
POST /password/forgot
POST /password/reset
GET /google/authorize
GET /google/callback
GET /apple/authorize
POST /apple/callback
```

Admin routes remain in the separate admin Lambda and are out of this public router cleanup except for tests that prove they are not mounted behind the public `$default` route.

Remove from the public auth router:

```text
GET /:provider/authorize
GET /:provider/callback
POST /:provider/callback
```

Remove the legacy helper functions that only support those routes:

```text
provider_authorize_handler
provider_callback_get_handler
provider_callback_post_handler
issue_auth_code_and_redirect
extract_session_key
mask_destination
CallbackForm
```

If any helper is still needed by a target API route, move it to the target module that owns it instead of keeping it in `routes.rs`.

### API Module Symmetry

Align the public router with the `design/auth/api` tree.

Add the missing target wrapper:

```text
packages/functions/auth/src/api/oauth/authorize.rs
```

It may re-export the existing implementation initially:

```text
pub use crate::oauth::authorize::handle_authorize;
```

Then update the router to call:

```text
crate::api::oauth::authorize::handle_authorize
```

This keeps the slice focused while moving public route ownership toward the intended `api/oauth/*` structure. Do not move the full authorize implementation unless it is mechanical and does not broaden the diff.

### Provider Config Removal

Remove the legacy runtime provider registry from application state.

Remove:

```text
ProviderConfig
AppState.providers
load_providers_from_env
PROVIDERS env parsing
PROVIDER_* env parsing
```

Tests and runtime state constructors should no longer pass:

```text
providers: Arc<HashMap<String, ProviderConfig>>
```

Target provider configuration remains in:

```text
RuntimeAuthConfig.google
RuntimeAuthConfig.apple
password runtime config and email/password modules
```

### Legacy Provider Module Deletion

Delete the legacy singular provider tree after all references are removed:

```text
packages/functions/auth/src/provider/
```

This removes:

```text
generic OAuth2 provider code
generic OIDC provider code
legacy password hosted-form provider
passwordless code provider
GitHub provider
provider traits used only by legacy dispatch
```

Keep the target plural provider tree:

```text
packages/functions/auth/src/providers/
```

### Built-In HTML UI Removal

Remove built-in HTML auth UI modules from the public auth Lambda:

```text
packages/functions/auth/src/ui/
```

The target core remains API-only. Email templates are still in scope and must not be removed.

Do not remove:

```text
packages/functions/auth/src/email/*
```

### Deployment Environment Cleanup

Remove generic provider env forwarding from SST infra.

Remove:

```text
PROVIDERS
PROVIDER_*
```

from:

```text
infra/api.ts
```

Target provider config uses explicit settings:

```text
AUTH_GOOGLE_CLIENT_ID
AUTH_GOOGLE_CLIENT_SECRET
AUTH_APPLE_CLIENT_ID
AUTH_APPLE_TEAM_ID
AUTH_APPLE_KEY_ID
AUTH_APPLE_PRIVATE_KEY_SECRET
```

Those settings should continue to flow through the existing `AUTH_*` environment forwarding or later secret binding work.

### Static Legacy-Surface Validation

Add or extend validation so the deleted legacy surface cannot quietly return.

Suggested target:

```text
scripts/validate-legacy-removal.mjs
```

or extend:

```text
scripts/validate-infra-routes.mjs
```

Required static checks:

- `packages/functions/auth/src/provider` does not exist.
- `packages/functions/auth/src/ui` does not exist.
- `routes.rs` does not mount `/:provider/authorize`.
- `routes.rs` does not mount `/:provider/callback`.
- `config.rs` does not define `ProviderConfig`.
- `AppState` has no `providers` field.
- `main.rs` does not define `load_providers_from_env`.
- `infra/api.ts` does not forward `PROVIDERS` or `PROVIDER_*`.
- `lib.rs` and `main.rs` do not export or mount `provider` or `ui` modules.

Add the validation command to `npm run test:infra` if a new script is created.

## Out Of Scope

- Removing `StorageAdapter`.
- Removing local in-process test storage.
- Replacing every remaining direct `StorageAdapter` call with typed store operations.
- Removing unmounted legacy admin/client-management modules.
- Removing old DynamoDB JWT signing-key helper modules if still unused but compiled.
- Removing OAuth refresh legacy helper functions inside `oauth/token.rs` unless they are directly tied to `src/provider` or UI routes.
- AWS live deployment testing.
- Hosted UI replacement.
- Generic OIDC provider support.
- Passwordless OTP or magic-link login.
- Payment or dashboard work.

## Expected Code Shape

Likely Rust changes:

```text
packages/functions/auth/src/routes.rs
packages/functions/auth/src/config.rs
packages/functions/auth/src/main.rs
packages/functions/auth/src/lib.rs
packages/functions/auth/src/api/oauth/mod.rs
packages/functions/auth/src/api/oauth/authorize.rs
packages/functions/auth/tests/runtime_route_slice.rs
packages/functions/auth/tests/*_slice.rs where AppState is constructed
```

Likely deletions:

```text
packages/functions/auth/src/provider/
packages/functions/auth/src/ui/
```

Likely infra/script changes:

```text
infra/api.ts
scripts/validate-infra-routes.mjs
package.json
```

If the route/static validation is easier to keep separate, create:

```text
scripts/validate-legacy-removal.mjs
```

and call it from:

```text
npm run test:infra
```

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add static validation tests that fail while `src/provider`, `src/ui`, dynamic provider routes, and `PROVIDER_*` forwarding still exist.
2. Add route tests proving unknown dynamic provider paths return `404 Not Found`.
3. Add `api/oauth/authorize.rs` wrapper and route `/authorize` through the target API module path.
4. Remove dynamic `/:provider/*` route mounts from `routes.rs`.
5. Remove legacy provider callback/authorize handlers and helper functions from `routes.rs`.
6. Remove `ProviderConfig` and `AppState.providers`.
7. Update all `AppState` construction sites in tests and code.
8. Remove `load_providers_from_env` and provider env parsing from `main.rs`.
9. Remove `PROVIDERS` / `PROVIDER_*` forwarding from `infra/api.ts`.
10. Delete `src/provider/` and remove `mod provider` / `pub mod provider`.
11. Delete `src/ui/` and remove `mod ui` / `pub mod ui`.
12. Run focused route and startup tests.
13. Run full auth tests, `cargo check`, `npm run typecheck`, `npm run test:infra`, and `npm run test:setup`.

## Tests

### Route Tests

- `GET /unknown/authorize?session=x` returns `404 Not Found`.
- `GET /unknown/callback?code=x&state=y` returns `404 Not Found`.
- `POST /unknown/callback` returns `404 Not Found`.
- `/authorize` still creates the target authorize session.
- `/password/register`, `/password/verify`, `/password/login`, `/password/forgot`, and `/password/reset` still work through API-only handlers.
- `/google/authorize` and `/google/callback` still use the first-class Google route tests.
- `/apple/authorize` and `/apple/callback` still use the first-class Apple route tests.
- Public bootstrap/client-management routes remain unmounted.
- Public auth router responses do not render built-in login, registration, code, or provider-selection forms.

### Config And Startup Tests

- `RuntimeAuthConfig` still loads password/email/Google/Apple/client config.
- `AppState` can be constructed without a provider registry.
- No startup code reads `PROVIDERS` or `PROVIDER_*`.
- Existing Google/Apple optional config behavior remains unchanged.

### Static Validation Tests

- `src/provider` is absent.
- `src/ui` is absent.
- `ProviderConfig` is absent.
- `load_providers_from_env` is absent.
- Dynamic `/:provider/*` route strings are absent.
- `PROVIDERS` and `PROVIDER_` forwarding are absent from SST infra.
- `provider` and `ui` modules are not exported from `lib.rs` or mounted in `main.rs`.

### Regression Tests To Keep Green

- Password registration does not issue auth codes or tokens.
- Password login is the first password route that can issue an authorization code.
- Google and Apple identity keys use issuer plus subject, not email.
- Provider state is HMAC-keyed and single-use.
- Discovery advertises only implemented flows and ES256.
- `/token` still rejects `client_credentials`.
- `/oauth/revoke` still works for user-facing logout.
- Admin lifecycle routes remain IAM-protected in the separate admin Lambda.

## Acceptance Criteria

- The public auth Lambda is API-only.
- No dynamic provider routes are mounted.
- `src/provider/` is deleted.
- `src/ui/` is deleted.
- `ProviderConfig` and `AppState.providers` are removed.
- `load_providers_from_env` is removed.
- SST no longer forwards `PROVIDERS` or `PROVIDER_*`.
- Password, Google, Apple, OAuth token, userinfo, discovery, JWKS, revoke, and admin lifecycle tests remain green.
- Static validation prevents the legacy provider/UI surface from returning accidentally.
- No hosted login, registration, code-entry, provider-selection, account-selection, or consent UI remains in the auth Lambda.

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

AWS validation is not required for this slice because it removes code paths and env forwarding rather than changing API Gateway auth, IAM, DynamoDB, KMS, or external provider behavior. AWS smoke testing can wait for the final cleanup/security-regression slice or the next dev deploy milestone.

## Next Slice

After this slice, implement `18_legacy_storage_and_security_regression`.

That slice should remove or quarantine the remaining target-incompatible legacy storage/admin/signing helpers, remove local in-process storage as a runtime option, and add final security regression checks around raw bearer values, runtime scans, and public control-plane routes.
