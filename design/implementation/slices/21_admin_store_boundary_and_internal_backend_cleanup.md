# 21_admin_store_boundary_and_internal_backend_cleanup

## Goal

Move IAM-protected admin lifecycle routes behind the same typed `AuthStore` boundary as the public auth Lambda, then tighten the remaining backend visibility rules.

At the end of this slice, no route handler should receive generic raw storage. Public auth routes and IAM admin routes should both depend on `AuthStore` typed operations. Raw backend methods should remain available only to store internals and test support.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/admin.md`
- `design/auth/core/account-lifecycle.md`
- `design/auth/store/accounts.md`
- `design/auth/store/identities.md`
- `design/auth/store/password-users.md`
- `design/auth/store/password-secrets.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/store/dynamodb.md`
- `design/auth/store/records.md`
- `design/auth/observability/audit.md`
- `design/auth/testing.md`
- `design/migration.md`
- `design/implementation/slices/20_store_boundary_and_in_memory_test_backend.md`

The important constraint is that admin lifecycle routes are still security-sensitive runtime routes. They must use typed account, identity, password, refresh, and audit operations rather than being a long-term exception to the store boundary.

## Why This Slice Next

Slice 20 removed raw storage from the public auth route/provider boundary, but intentionally left `api/admin.rs` as a scoped exception. That file still uses:

```rust
AdminAppState<S: StorageAdapter> {
    storage: Arc<S>,
}
```

and recreates `AuthStore` inside each handler:

```rust
let store = AuthStore::new(app.storage.clone());
```

It also records audit events by passing raw storage into `audit::record_event`. This slice should eliminate that exception before deeper store cleanup or AWS smoke testing.

## Scope Decision

This is a boundary cleanup slice, not an admin behavior slice.

In scope:

- `api/admin.rs`
- admin integration tests
- `scripts/validate-store-boundary.mjs`
- `AuthStore` audit helper use from admin routes
- minimal backend visibility cleanup that follows naturally
- the existing `subject::schema::*` unused warning if it is still present

Out of scope:

- new admin endpoints
- changing IAM authorizer behavior
- changing account lifecycle semantics
- changing deleted identity reuse behavior
- changing DynamoDB table shape
- replacing all internal scans
- AWS live deployment testing

## Target Shape

Admin state should mirror the public state boundary:

```rust
#[derive(Clone)]
pub struct AdminAppState {
    pub store: AuthStore,
    pub lifecycle: AccountLifecycleConfig,
}
```

Admin router construction should be non-generic:

```rust
pub fn create_admin_router(state: AdminAppState) -> Router
```

Admin handlers should call typed store methods directly:

```rust
let account = app.store.get_account(&subject).await?;
let revoked = app.store.revoke_refresh_tokens_for_subject(subject.as_str()).await?;
let _ = app.store.record_audit_event(event).await;
```

No admin route should import `StorageAdapter`, hold raw storage, call `.storage`, or instantiate `AuthStore` from a backend inside a handler.

## In Scope

### AdminAppState Cutover

Change `AdminAppState<S>` to non-generic `AdminAppState`.

Before:

```rust
pub struct AdminAppState<S: StorageAdapter> {
    pub storage: Arc<S>,
    pub lifecycle: AccountLifecycleConfig,
}
```

After:

```rust
pub struct AdminAppState {
    pub store: AuthStore,
    pub lifecycle: AccountLifecycleConfig,
}
```

Update admin test builders to construct:

```rust
AdminAppState {
    store: AuthStore::new(TestStorage::new()),
    lifecycle: AccountLifecycleConfig::default(),
}
```

Where tests need raw storage inspection, keep a cloned `TestStorage` handle in the test helper. Do not add raw storage back to `AdminAppState`.

### Admin Router And Handler Cleanup

Remove `S: StorageAdapter` from:

- `create_admin_router`
- `get_user`
- `disable_user`
- `delete_user`
- `revoke_user_sessions`

Remove handler-local store reconstruction:

```rust
let store = AuthStore::new(app.storage.clone());
```

Use `app.store` directly.

### Admin Audit Through AuthStore

Replace:

```rust
audit::record_event(app.storage.as_ref(), event).await
```

with:

```rust
app.store.record_audit_event(event).await
```

`audit.rs` may remain a low-level helper called by `AuthStore`. Route handlers should not receive raw storage to emit audit records.

### Store-Boundary Validator Tightening

Update:

```text
scripts/validate-store-boundary.mjs
```

so it includes `packages/functions/auth/src/api/admin.rs` in the route-boundary checks.

Required checks after this slice:

- `api/admin.rs` does not import `StorageAdapter`.
- `api/admin.rs` does not contain `.storage`.
- `api/admin.rs` does not contain `AppState<S>` or `AdminAppState<S>`.
- `api/admin.rs` does not contain `<S: StorageAdapter>`.
- `api/admin.rs` does not call `AuthStore::new` inside handlers.

Keep allowed raw storage areas limited to:

- `packages/functions/auth/src/store/**`
- `packages/functions/auth/src/storage/**`
- `packages/functions/auth/tests/support/**`
- integration tests that intentionally inspect test backend state

### Backend Visibility Follow-Up

After admin is migrated, check whether the crate still exposes backend pluggability unnecessarily.

Expected target:

- top-level `lib.rs` does not re-export `StorageAdapter`
- route/admin/provider modules do not import `StorageAdapter`
- `StorageAdapter` remains available to store/storage/test-support code

If making `StorageAdapter` `pub(crate)` is mechanical and does not break integration-test support, do it in this slice. If it requires a larger test-support redesign, leave it for the next slice and document why.

### Warning Cleanup

If the existing warning remains:

```text
unused import: schema::*
```

clean it up in this slice, as long as it is a small export/import adjustment and does not change public behavior.

## Out Of Scope

- Removing `StorageAdapter` completely.
- Removing the in-memory test backend.
- Adding DynamoDB Local.
- Rewriting account deletion internals.
- Replacing internal scan calls with DynamoDB Query-shaped helpers.
- Adding admin client management or runtime config APIs.
- Changing IAM deployment shape.
- AWS smoke testing.

## Expected Code Shape

Likely modified files:

```text
packages/functions/auth/src/api/admin.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/audit.rs
packages/functions/auth/src/lib.rs
packages/functions/auth/src/subject/mod.rs
packages/functions/auth/tests/admin_lifecycle.rs
packages/functions/auth/tests/admin_deletion.rs
scripts/validate-store-boundary.mjs
```

Only touch store/audit files if the admin cutover needs a missing typed helper.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Update `scripts/validate-store-boundary.mjs` to include `api/admin.rs` and fail on the current admin raw-storage shape.
2. Run `npm run test:infra` and confirm the new validator fails for admin route boundary violations.
3. Change `AdminAppState` to hold `store: AuthStore` and remove the generic storage parameter.
4. Change `create_admin_router` and all admin handlers to non-generic functions.
5. Replace handler-local `AuthStore::new(app.storage.clone())` with `app.store`.
6. Replace admin audit writes with `app.store.record_audit_event(event).await`.
7. Update admin integration tests to build `AdminAppState { store: AuthStore::new(storage.clone()), ... }`.
8. Preserve raw test storage inspection by keeping cloned `TestStorage` handles only in tests.
9. Run focused admin tests:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_lifecycle
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_deletion
```

10. Re-run `npm run test:infra`; the store-boundary validator should pass with admin included.
11. Remove the `subject::schema::*` unused warning if still present.
12. Run full verification:

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

- `npm run test:infra` fails before admin migration because `api/admin.rs` imports `StorageAdapter`.
- `npm run test:infra` fails before admin migration because `api/admin.rs` uses `.storage`.
- `npm run test:infra` passes after admin route code depends only on `AuthStore`.
- The validator still allows raw backend calls inside store/storage/test-support.

### Admin Behavior Tests

Existing admin tests must still prove:

- admin routes reject missing IAM request context
- custom admin keys do not authenticate admin routes
- admin account read returns sanitized account state
- disabling an account marks it inactive and revokes sessions
- disabling is idempotent for already disabled accounts
- deleted accounts cannot be disabled or restored
- session revocation does not disable the account
- deletion tombstones account-owned auth state
- deletion revokes refresh-token families
- deleted identity reuse policy is preserved

No new admin behavior should be added in this slice.

## Acceptance Criteria

- `AdminAppState` is non-generic and holds `store: AuthStore`.
- `create_admin_router` is non-generic.
- Admin handlers do not import `StorageAdapter`.
- Admin handlers do not call `.storage`.
- Admin handlers do not reconstruct `AuthStore` from raw storage.
- Admin audit events are emitted through `AuthStore`.
- `validate-store-boundary.mjs` checks `api/admin.rs`.
- No route handler in public or admin auth code exposes raw storage.
- Existing admin behavior tests pass.
- Full verification passes.
- The existing unused `subject::schema::*` warning is removed if mechanically possible.

## Manual Validation

No AWS validation is required for this slice. It changes the Rust route boundary only.

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

After this slice, define `22_internal_store_query_and_backend_visibility_cleanup`.

That slice should decide whether to:

- make `StorageAdapter` crate-private
- move test storage behind explicit test-support helpers
- replace remaining runtime auth `scan` calls with bounded query-shaped typed operations where the design requires it
- prepare the AWS dev smoke-test checklist for the now-simplified runtime boundary
