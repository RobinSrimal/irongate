# 22_internal_store_query_and_backend_visibility_cleanup

## Goal

Make the remaining internal storage traversal paths explicitly query-shaped, then tighten backend visibility guardrails without changing auth behavior.

At the end of this slice, production Rust code should no longer call a method named `scan`. Runtime paths that enumerate related records should call bounded partition-query helpers. The raw backend abstraction may still exist for DynamoDB and test backends, but production route/provider/admin code must continue to see only `AuthStore`.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/store/dynamodb.md`
- `design/auth/store/records.md`
- `design/auth/store/keys.md`
- `design/auth/store/accounts.md`
- `design/auth/store/identities.md`
- `design/auth/store/password-users.md`
- `design/auth/store/password-secrets.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/observability/audit.md`
- `design/auth/testing.md`
- `design/migration.md`
- `design/implementation/slices/20_store_boundary_and_in_memory_test_backend.md`
- `design/implementation/slices/21_admin_store_boundary_and_internal_backend_cleanup.md`

The important design constraint is that the physical DynamoDB table can stay simple, but runtime auth paths should be exact-key, conditional-write, transaction, or bounded partition-query paths. A method named `scan` makes that boundary harder to reason about even when the DynamoDB implementation currently uses `Query`.

## Why This Slice Next

Slice 20 moved public route/provider code behind `AuthStore`. Slice 21 moved IAM admin lifecycle routes behind the same boundary. The remaining storage cleanup is now internal:

```text
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/password_users.rs
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/store/refresh.rs
```

These files still call:

```rust
self.storage.scan(&[partition_key.as_str()]).await?
```

The current DynamoDB adapter implements that as a partition `Query`, not a table scan. The next step is to make the API name and validation match the intended AWS access pattern.

## Scope Decision

This is a storage-boundary cleanup slice, not a data-model rewrite.

In scope:

- Rename the backend traversal method used by production code from `scan` to a bounded query name.
- Keep query semantics limited to one encoded partition key plus optional sort-key prefix.
- Update DynamoDB and in-memory test backend implementations.
- Update store internals to call the query-named method.
- Update tests that inspect storage directly to use the query-named test helper.
- Add static validation that production Rust code does not call `.scan(`.
- Audit `StorageAdapter` public visibility and document why it remains public if integration tests still need to implement it.

Out of scope:

- Removing `StorageAdapter` completely.
- Making `StorageAdapter` crate-private if that requires redesigning all integration test backends.
- Replacing all integration-test storage inspection with domain-specific assertion helpers.
- Adding DynamoDB Local.
- Adding new auth behavior.
- Changing account deletion, refresh revocation, or password reset semantics.
- Changing the DynamoDB table shape.
- AWS live deployment testing.

## Target Shape

Rename the raw traversal API so the contract matches the AWS access pattern:

```rust
async fn query_prefix(&self, prefix: &[&str]) -> Result<Vec<(Vec<String>, Value)>, StorageError>;

async fn query_prefix_page(
    &self,
    prefix: &[&str],
    limit: u32,
    cursor: Option<&str>,
) -> Result<(Vec<(Vec<String>, Value)>, Option<String>), StorageError>;
```

The DynamoDB implementation should continue to use:

```text
pk = :pk
pk = :pk AND begins_with(sk, :sk)
```

The in-memory test backend should continue to use ordered key ranges to simulate the same bounded-prefix behavior.

Production store code should read like:

```rust
let rows = self.storage.query_prefix(&[index_pk.as_str()]).await?;
```

not:

```rust
let rows = self.storage.scan(&[index_pk.as_str()]).await?;
```

## In Scope

### Backend Method Rename

Update:

```text
packages/functions/auth/src/storage/adapter.rs
packages/functions/auth/src/storage/dynamo.rs
packages/functions/auth/src/storage/test_support.rs
packages/functions/auth/tests/support/mod.rs
```

Rename trait methods:

```text
scan -> query_prefix
scan_page -> query_prefix_page
```

Keep method behavior unchanged:

- Same key-prefix encoding.
- Same expiry filtering.
- Same return shape.
- Same pagination cursor behavior.
- Same DynamoDB `Query` expression.

Do not add a real DynamoDB `Scan` operation.

### Store Internal Call Sites

Update production store internals:

```text
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/password_users.rs
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/store/refresh.rs
```

Expected call-site changes:

```rust
self.storage.scan(&[index_pk.as_str()]).await?
```

becomes:

```rust
self.storage.query_prefix(&[index_pk.as_str()]).await?
```

These paths are intentionally bounded because the partition key is subject-scoped:

- `identity_by_subject:<subject>`
- `password_user_by_subject:<subject>`
- `password_reset_by_subject:<subject>`
- `refresh_by_subject:<subject>`

### Test Inspection Updates

Update integration tests that inspect raw storage directly:

```text
packages/functions/auth/tests/api_routes.rs
packages/functions/auth/tests/oauth_token_userinfo.rs
packages/functions/auth/tests/password_login.rs
packages/functions/auth/tests/password_reset.rs
packages/functions/auth/tests/oidc_google_start.rs
packages/functions/auth/tests/oidc_google_callback.rs
packages/functions/auth/tests/oidc_apple_start.rs
packages/functions/auth/tests/oidc_apple_callback.rs
```

Change test inspection from:

```rust
storage.scan(&["oauth:code"]).await
```

to:

```rust
storage.query_prefix(&["oauth:code"]).await
```

Do not broaden tests in this slice. The test behavior should stay the same; only the storage inspection method name changes.

### Static Validation

Update:

```text
scripts/validate-store-boundary.mjs
```

Add a production-source check that fails on `.scan(` outside storage adapter implementation files and test files.

Recommended check shape:

```text
packages/functions/auth/src/**/*.rs
exclude packages/functions/auth/src/storage/**
exclude #[cfg(test)] module bodies
fail if /\.scan\s*\(/
```

The validator should still allow:

- `query_prefix` in store internals.
- test-only storage inspection in `packages/functions/auth/tests/**`.
- adapter implementation comments that explain there is no DynamoDB table scan.

### Visibility Audit

Check whether `StorageAdapter` can become crate-private after the method rename.

If it is mechanical, make this change:

```rust
pub(crate) use adapter::{StorageAdapter, TransactCondition, TransactOperation};
```

Only do that if integration tests can still build without introducing a larger test-support redesign.

If integration tests still need to implement `StorageAdapter`, leave the trait public and add a short comment to `storage/mod.rs` explaining:

```text
StorageAdapter is public for integration-test backends only.
Runtime route/provider/admin code must not import it; this is enforced by scripts/validate-store-boundary.mjs.
```

Do not introduce a public runtime storage plugin story.

## Out Of Scope

- New typed account lifecycle behavior.
- New auth endpoints.
- New OAuth/OIDC flows.
- New DynamoDB indexes.
- New IAM policies.
- KMS signing or KMS table encryption changes.
- Replacing test inspection with a complete assertion-helper layer.
- AWS smoke testing.

## Expected Code Shape

Likely modified files:

```text
packages/functions/auth/src/storage/adapter.rs
packages/functions/auth/src/storage/dynamo.rs
packages/functions/auth/src/storage/mod.rs
packages/functions/auth/src/storage/test_support.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/password_users.rs
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/store/refresh.rs
packages/functions/auth/tests/support/mod.rs
packages/functions/auth/tests/api_routes.rs
packages/functions/auth/tests/oauth_token_userinfo.rs
packages/functions/auth/tests/password_login.rs
packages/functions/auth/tests/password_reset.rs
packages/functions/auth/tests/oidc_google_start.rs
packages/functions/auth/tests/oidc_google_callback.rs
packages/functions/auth/tests/oidc_apple_start.rs
packages/functions/auth/tests/oidc_apple_callback.rs
scripts/validate-store-boundary.mjs
```

Do not touch auth route behavior unless compilation requires a method-name update.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add or update validator coverage so production `.scan(` calls fail outside storage adapter code.
2. Run `npm run test:infra` and confirm the new validator fails on existing production store `.scan(` call sites.
3. Rename `StorageAdapter::scan` to `query_prefix`.
4. Rename `StorageAdapter::scan_page` to `query_prefix_page`.
5. Update the `Arc<T>` trait forwarding implementation.
6. Update `DynamoStorage` to implement `query_prefix` and `query_prefix_page` with the existing DynamoDB `Query` logic.
7. Update `storage::test_support::TestStorage` to implement the renamed methods with the same ordered-range behavior.
8. Update `tests/support::TestStorage` to implement the renamed methods with the same ordered-range behavior.
9. Update production store call sites to use `query_prefix`.
10. Update integration-test raw-storage inspections to use `query_prefix`.
11. Run focused tests that cover each remaining production query path:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_deletion
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_lifecycle
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oauth_refresh_revoke
cargo test --manifest-path packages/functions/auth/Cargo.toml --test password_reset
```

12. Re-run `npm run test:infra`; the store-boundary validator should pass with no production `.scan(` call sites.
13. Check whether `StorageAdapter` can be made crate-private without large test-support redesign.
14. If crate-private is mechanical, apply it and run the focused tests again.
15. If crate-private is not mechanical, leave it public with the integration-test-only comment in `storage/mod.rs`.
16. Run full verification:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run test:infra
npm run typecheck
npm run test:setup
git diff --check
```

## Tests

### Static Boundary Tests

- `npm run test:infra` fails before the rename because production store code calls `.scan(`.
- `npm run test:infra` passes after production store code calls `query_prefix`.
- `scripts/validate-store-boundary.mjs` still rejects `StorageAdapter` imports from route/provider/admin files.
- `scripts/validate-store-boundary.mjs` still rejects route-local `AuthStore::new(...)`.

### Runtime Behavior Tests

Existing tests must continue to prove:

- Admin deletion tombstones account-owned state and revokes sessions.
- Admin disable and revoke-session routes revoke indexed refresh families.
- Refresh-token subject revocation is bounded to subject index records.
- Password reset deletion removes subject-indexed reset records.
- Expired records are filtered before being returned.
- Raw bearer values are not stored in DynamoDB keys.

No new auth behavior should be added in this slice.

## Acceptance Criteria

- Production Rust code outside `storage/**` no longer calls `.scan(`.
- Store internals use `query_prefix` for bounded partition traversal.
- DynamoDB implementation still uses `Query`, not table `Scan`.
- In-memory test backends simulate the same query-prefix behavior.
- Integration tests compile without importing a runtime storage plugin model into routes.
- `validate-store-boundary.mjs` enforces the production no-scan naming rule.
- `StorageAdapter` visibility is either narrowed mechanically or explicitly documented as public for integration-test backends only.
- Full verification passes.

## Manual Validation

No AWS validation is required for this slice. It changes Rust storage naming and static validation only.

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run test:infra
npm run typecheck
npm run test:setup
git diff --check
```

## Next Slice

After this slice, define `23_aws_dev_smoke_test_checklist_and_deploy_validation`.

That slice should prepare and run the first AWS dev validation pass for:

- API Gateway source IP behavior.
- IAM-protected admin Lambda behavior.
- DynamoDB key shape for auth codes, provider state, refresh tokens, verification, and reset records.
- TTL attributes on short-lived records.
- CloudWatch logging and audit mode defaults.
- SST dev/prod stage/account assumptions.
