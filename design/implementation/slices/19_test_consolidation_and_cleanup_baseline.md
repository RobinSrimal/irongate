# 19_test_consolidation_and_cleanup_baseline

## Goal

Consolidate the auth crate tests from implementation-slice files into Rust-conventional module and domain test locations, without changing auth behavior.

At the end of this slice, tests should describe what they verify rather than when they were introduced. Pure module tests should live beside the module where practical, and end-to-end protocol/router tests should remain under `packages/functions/auth/tests/` with stable domain names.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/testing.md`
- `design/auth/threat-model.md`
- `design/implementation/ROADMAP.md`
- `design/implementation/slices/18_legacy_storage_and_security_regression.md`

The important constraints are:

- Do not change runtime behavior.
- Do not weaken or delete security regression coverage.
- Do not add new auth features.
- Keep moves mechanical and reviewable.
- Preserve the existing test support helpers unless a smaller helper extraction is obviously mechanical.

## Why This Slice Next

The implementation slices are now mostly complete, but the test tree still reflects the construction history:

```text
foundation_slice.rs
password_registration_slice.rs
password_login_slice.rs
token_exchange_slice.rs
refresh_logout_slice.rs
...
```

That was useful while implementing the rewrite, but it is a poor long-term shape. Rust maintainers should be able to find tests by module or behavior:

```text
src/crypto/signing.rs
src/config/client_file.rs
tests/oauth_token.rs
tests/password.rs
tests/oidc_google.rs
```

This slice turns the test suite into a maintainable baseline before AWS deployment validation.

## Test Organization Decision

Use both Rust test styles intentionally:

### Source-Local Unit Tests

Use `#[cfg(test)] mod tests` in the source module when the tests exercise a focused pure module or private helper behavior.

Good candidates:

```text
packages/functions/auth/src/config/account_lifecycle.rs
packages/functions/auth/src/config/audit.rs
packages/functions/auth/src/config/client_file.rs
packages/functions/auth/src/config/signing.rs
packages/functions/auth/src/config/ttls.rs
packages/functions/auth/src/crypto/hmac_lookup.rs
packages/functions/auth/src/crypto/kms_signing.rs
packages/functions/auth/src/crypto/signing.rs
packages/functions/auth/src/ratelimit/middleware.rs
packages/functions/auth/src/store/keys.rs
```

### Integration Tests

Keep integration tests under `packages/functions/auth/tests/` when they exercise:

- Axum routers.
- OAuth protocol flow across several modules.
- Account lifecycle behavior through public/admin APIs.
- End-to-end storage state across multiple typed store modules.
- Provider callbacks with fake OIDC clients.

Name integration files by domain, not slice number.

## In Scope

### Rename And Consolidate Integration Test Files

Replace slice-named files with domain-named files.

Target mapping:

| Current file | Target file |
| --- | --- |
| `runtime_route_slice.rs` | `api_routes.rs` |
| `token_exchange_slice.rs` | `oauth_token_userinfo.rs` |
| `refresh_logout_slice.rs` | `oauth_refresh_revoke.rs` |
| `password_registration_slice.rs` | `password_registration.rs` or `password.rs` |
| `password_login_slice.rs` | `password_login.rs` or `password.rs` |
| `password_reset_slice.rs` | `password_reset.rs` or `password.rs` |
| `google_oidc_start_slice.rs` | `oidc_google_start.rs` or `oidc_google.rs` |
| `google_oidc_callback_slice.rs` | `oidc_google_callback.rs` or `oidc_google.rs` |
| `apple_oidc_start_slice.rs` | `oidc_apple_start.rs` or `oidc_apple.rs` |
| `apple_oidc_callback_slice.rs` | `oidc_apple_callback.rs` or `oidc_apple.rs` |
| `admin_lifecycle_slice.rs` | `admin_lifecycle.rs` |
| `admin_deletion_slice.rs` | `admin_deletion.rs` or `admin_lifecycle.rs` |
| `startup_config_slice.rs` | `startup_config.rs` |
| `rate_limit_source_slice.rs` | `aws_request_context.rs` |
| `kms_signing_slice.rs` | source-local `crypto/kms_signing.rs` tests if practical, otherwise `kms_signing.rs` |
| `foundation_slice.rs` | split into source-local tests and/or `foundation.rs` |

Do not force giant files. If merging Google start and callback creates a file that is hard to review, keep two domain files:

```text
oidc_google_start.rs
oidc_google_callback.rs
```

The same applies to Apple and password.

### Move Pure Tests Into Source Modules

Move pure tests out of `foundation_slice.rs` when they directly belong to one module:

- Discovery metadata tests can move to `oauth/well_known.rs`.
- TTL/account lifecycle/audit config tests can move to their config modules.
- Client-file validation tests can move to `config/client_file.rs`.
- HMAC key helper tests can move to `crypto/hmac_lookup.rs` or `store/keys.rs`.
- Local ES256 JWKS tests can move to `crypto/signing.rs`.
- KMS DER conversion tests can move to `crypto/kms_signing.rs`.
- API Gateway source identity tests can move to `ratelimit/middleware.rs`.

If a test needs `tests/support`, router construction, fake email senders, or multi-module setup, keep it as an integration test.

### Keep Shared Integration Support

Keep:

```text
packages/functions/auth/tests/support/mod.rs
```

but review it for naming and duplication.

Allowed mechanical cleanup:

- Rename helpers for domain clarity.
- Remove helpers that become unused after file consolidation.
- Keep `NoopEmailSender` and `TestStorage` available to integration tests.

Do not rewrite the storage test helper in this slice.

### Add A Test Layout Note

Create:

```text
packages/functions/auth/tests/README.md
```

Document:

- Unit tests live beside pure modules.
- Integration tests live under `tests/` and are named by auth domain.
- Files must not be named after implementation slices.
- `support/mod.rs` is shared integration-test infrastructure.
- Security regression tests should be named by risk or behavior, not by old finding IDs unless that ID is useful context inside the test body.

### Remove Slice Naming

After moving tests, there should be no integration test file matching:

```text
*_slice.rs
```

Add a static check to prevent reintroducing slice-named integration tests.

Suggested location:

```text
scripts/validate-test-layout.mjs
```

Call it from:

```text
npm run test:infra
```

Required checks:

- `packages/functions/auth/tests/*_slice.rs` does not exist.
- `packages/functions/auth/tests/support/mod.rs` may exist.
- `packages/functions/auth/tests/README.md` exists.
- Every `packages/functions/auth/tests/*.rs` file name is domain-oriented and does not start with a numeric slice prefix.

## Out Of Scope

- Changing auth behavior.
- Adding new AWS deployment tests.
- Removing `StorageAdapter`.
- Rewriting `TestStorage`.
- Changing DynamoDB production code.
- Broad formatting of untouched source files.
- Reducing test coverage to make consolidation easier.
- Creating snapshot tests.

## Expected Code Shape

Likely source test moves:

```text
packages/functions/auth/src/config/*        #[cfg(test)] modules
packages/functions/auth/src/crypto/*        #[cfg(test)] modules
packages/functions/auth/src/oauth/well_known.rs
packages/functions/auth/src/ratelimit/middleware.rs
packages/functions/auth/src/store/keys.rs
```

Likely integration test files after consolidation:

```text
packages/functions/auth/tests/README.md
packages/functions/auth/tests/api_routes.rs
packages/functions/auth/tests/admin_deletion.rs
packages/functions/auth/tests/admin_lifecycle.rs
packages/functions/auth/tests/oauth_refresh_revoke.rs
packages/functions/auth/tests/oauth_token_userinfo.rs
packages/functions/auth/tests/oidc_apple_callback.rs
packages/functions/auth/tests/oidc_apple_start.rs
packages/functions/auth/tests/oidc_google_callback.rs
packages/functions/auth/tests/oidc_google_start.rs
packages/functions/auth/tests/password_login.rs
packages/functions/auth/tests/password_registration.rs
packages/functions/auth/tests/password_reset.rs
packages/functions/auth/tests/startup_config.rs
packages/functions/auth/tests/support/mod.rs
```

If smaller files are preferable during implementation, use the split names above. Do not merge all password/OIDC/admin tests into large files just to reduce file count.

Likely scripts:

```text
scripts/validate-test-layout.mjs
package.json
```

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add `scripts/validate-test-layout.mjs` that fails while `*_slice.rs` files exist.
2. Add `packages/functions/auth/tests/README.md` with the target test layout rules.
3. Run `npm run test:infra` and confirm the new layout validation fails because slice-named test files still exist.
4. Rename integration test files mechanically to domain names without editing test bodies.
5. Run the renamed integration tests by exact file target where practical.
6. Move the smallest pure tests from `foundation_slice.rs` into source-local `#[cfg(test)]` modules.
7. Run the source module tests that received moved tests.
8. Repeat pure-test moves only while each move stays mechanical and easy to review.
9. Remove now-empty slice files.
10. Remove unused imports or helpers caused by the moves.
11. Run `cargo test --manifest-path packages/functions/auth/Cargo.toml`.
12. Run `cargo check --manifest-path packages/functions/auth/Cargo.toml`.
13. Run `npm run test:infra`, `npm run typecheck`, `npm run test:setup`, and `git diff --check`.

## Tests

### Static Layout Tests

- `npm run test:infra` fails before consolidation when `*_slice.rs` files exist.
- `npm run test:infra` passes after consolidation.
- The layout validator accepts `tests/support/mod.rs`.
- The layout validator requires `tests/README.md`.

### Rust Tests

The full test command must still pass:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
```

Focused test commands should be run after each major move, for example:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test api_routes
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oauth_token_userinfo
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oauth_refresh_revoke
cargo test --manifest-path packages/functions/auth/Cargo.toml --test password_registration
cargo test --manifest-path packages/functions/auth/Cargo.toml --test password_login
cargo test --manifest-path packages/functions/auth/Cargo.toml --test password_reset
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_google_start
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_google_callback
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_apple_start
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_apple_callback
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_lifecycle
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_deletion
cargo test --manifest-path packages/functions/auth/Cargo.toml --test startup_config
```

If a file is merged or split differently, adjust only the command name, not the coverage expectation.

## Acceptance Criteria

- No `packages/functions/auth/tests/*_slice.rs` files remain.
- Integration test files are named by auth domain.
- Pure helper/config tests that can live beside their source module are moved there.
- `packages/functions/auth/tests/README.md` documents the convention.
- Static validation prevents slice-named integration tests from returning.
- All existing security regression coverage remains present.
- Full Rust tests, cargo check, infra validation, typecheck, setup tests, and diff check pass.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run test:infra
npm run typecheck
npm run test:setup
git diff --check
```

No AWS validation is required for this slice. It changes test organization only.

## Cleanup Backlog After This Slice

After test consolidation, the remaining cleanup questions are:

- Decide whether to remove or rename the generic `StorageAdapter` abstraction in favor of a DynamoDB-only concrete store.
- Decide whether to make a formatting baseline commit for the Rust crate.
- Remove the existing `subject::schema::*` unused import warning if it is still present.
- Run the AWS dev deployment smoke test described by the earlier infra slices.
- Revisit README/operator docs once AWS smoke testing confirms the final deployment shape.
