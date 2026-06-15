# 20_store_boundary_and_in_memory_test_backend

## Goal

Collapse public auth runtime storage access behind a non-generic typed `AuthStore`, while keeping the test suite fast with a simple in-memory backend.

At the end of this slice, public auth routes, OAuth handlers, provider API handlers, and provider domain code should depend on typed store operations. They should not receive `S: StorageAdapter`, should not recreate `AuthStore` from raw storage, and should not call raw `get`, `set`, `remove`, `scan`, or `transact` methods.

Production remains DynamoDB-only. The in-memory backend is for tests only.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/store/dynamodb.md`
- `design/auth/store/keys.md`
- `design/auth/store/records.md`
- `design/auth/store/authorization-codes.md`
- `design/auth/store/authorize-sessions.md`
- `design/auth/store/provider-states.md`
- `design/auth/store/password-secrets.md`
- `design/auth/store/password-users.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/store/rate-limits.md`
- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/api/providers/password.md`
- `design/auth/providers/google.md`
- `design/auth/providers/apple.md`
- `design/auth/testing.md`
- `design/migration.md`

The important design constraint is that generic raw storage is an internal store implementation detail. Route and provider code must use purpose-specific operations that preserve HMAC keying, TTL, one-time consume, refresh rotation, and account lifecycle invariants.

## Why This Slice Next

Slice 19 made the test tree maintainable. The next cleanup should remove the largest remaining architectural leak: public handlers still receive generic storage through `AppState<S>` and frequently do this:

```rust
let store = AuthStore::new(app.storage.clone());
```

That shape makes it easy for route code to bypass typed store invariants later. Moving to one typed store in state gives the next slices a simpler foundation before any more AWS validation or storage cleanup.

## Scope Decision

This slice should focus on the public auth Lambda boundary first.

In scope:

- `routes.rs`
- `api/oauth/*`
- `api/providers/*`
- `oauth/*`
- `providers/*`
- public `AppState`
- public auth integration tests
- test support needed by those tests

IAM admin routes may be migrated in this slice if the change is mechanical after the public state shape changes. If that starts to expand the diff significantly, leave `AdminAppState<S>` and `api/admin.rs` as the explicit follow-up exception and document that in the final notes.

Do not combine this with deeper DynamoDB query/index work.

## Target Shape

Runtime:

```text
public routes / API handlers / providers
  -> AppState
  -> AuthStore typed operations
  -> DynamoDB backend
```

Tests:

```text
public routes / API handlers / providers
  -> AppState
  -> AuthStore typed operations
  -> test-only in-memory backend
```

Target Rust shape:

```rust
#[derive(Clone)]
pub struct AppState {
    pub store: AuthStore,
    pub config: Arc<Config>,
    pub runtime: Arc<RuntimeAuthConfig>,
    pub email_sender: Arc<dyn VerificationEmailSender>,
    pub google_client: Arc<dyn GoogleOidcClient>,
    pub apple_client: Arc<dyn AppleOidcClient>,
}
```

and:

```rust
#[derive(Clone)]
pub struct AuthStore {
    backend: Arc<dyn StorageAdapter>,
}
```

`StorageAdapter` can remain as an internal backend trait during this slice, but public route/provider code should not import it or mention it in handler signatures.

## In Scope

### Non-Generic AuthStore

Change `AuthStore<S>` into a cloneable, non-generic `AuthStore` that owns an `Arc<dyn StorageAdapter>`.

Required constructors:

```rust
impl AuthStore {
    pub fn new<S>(backend: S) -> Self
    where
        S: StorageAdapter + 'static;

    pub(crate) fn from_backend(backend: Arc<dyn StorageAdapter>) -> Self;
}
```

Exact names can change during implementation, but the result should let production pass `DynamoStorage` and tests pass `TestStorage` without exposing the backend type to route handlers.

### Public AppState Cutover

Change public `AppState` from:

```rust
pub struct AppState<S: StorageAdapter> {
    pub storage: Arc<S>,
    ...
}
```

to:

```rust
pub struct AppState {
    pub store: AuthStore,
    ...
}
```

Then update:

- `main.rs` to construct `AuthStore::new(DynamoStorage::new(...))`.
- `routes.rs` to accept `AppState` without generic parameters.
- public OAuth and provider handlers to accept `State<AppState>`.
- public route registrations to stop using turbofish handler specialization.

### Public Handler Store Access

Replace public handler patterns like:

```rust
let store = AuthStore::new(app.storage.clone());
```

with:

```rust
let store = app.store.clone();
```

or direct calls through `app.store`.

Affected public auth areas:

- `api/oauth/authorize.rs`
- `api/oauth/token.rs`
- `api/oauth/revoke.rs`
- `api/oauth/userinfo.rs`
- `api/oauth/discovery.rs`
- `api/providers/password.rs`
- `api/providers/google.rs`
- `api/providers/apple.rs`
- `oauth/authorize.rs`
- `oauth/token.rs`
- `oauth/revoke.rs`
- `oauth/userinfo.rs`
- `oauth/well_known.rs`
- `providers/password.rs`
- `providers/google.rs`
- `providers/apple.rs`

If some `api/oauth/*` files only re-export legacy modules, update the actual implementation file rather than adding duplicate wrappers.

### Rate Limiting Through Typed Store

Public route middleware currently calls:

```rust
check_rate_limit(app.storage.as_ref(), ...)
```

Move this behind the typed store boundary.

Reasonable target:

```rust
app.store.check_rate_limit(&app.config.rate_limit, endpoint, &identifier).await
```

The low-level rate-limit implementation may remain in `store/rate_limits.rs` and may still use raw backend calls internally.

### Audit Through Typed Store

Public token and revoke flows currently call:

```rust
audit::record_event(state.storage.as_ref(), event).await
```

Move audit emission behind the store boundary or an explicitly typed audit sink.

Reasonable minimal target:

```rust
state.store.record_audit_event(event).await
```

`audit.rs` may remain as a low-level helper if only `AuthStore` calls it. Public handlers should not pass raw storage to audit helpers.

### Test Support

Keep a simple in-memory backend for tests.

Allowed shape:

```text
packages/functions/auth/tests/support/mod.rs
```

or:

```text
packages/functions/auth/src/storage/test_support.rs
```

The in-memory backend must remain test-only or clearly non-production. It should not be documented as a runtime deployment option.

Update integration-test builders so they construct `AppState` with:

```rust
store: AuthStore::new(TestStorage::new())
```

When tests need to assert raw key shape, expose that capability through test support only. Do not add raw storage back to production `AppState`.

### Public Export Cleanup

Remove public exports that make backend pluggability look like part of the runtime API.

Target direction:

```rust
pub use storage::DynamoStorage;
```

instead of:

```rust
pub use storage::{DynamoStorage, StorageAdapter};
```

If tests need the trait, import it through a test-only module or keep the trait `pub(crate)` and expose test helpers from `tests/support`.

### Static Validation

Add:

```text
scripts/validate-store-boundary.mjs
```

and call it from:

```text
npm run test:infra
```

Required checks:

- `packages/functions/auth/src/routes.rs` does not import `StorageAdapter`.
- `packages/functions/auth/src/routes.rs` does not contain `.storage`.
- Public handler files under `packages/functions/auth/src/api/oauth/*.rs` do not import `StorageAdapter`.
- Public handler files under `packages/functions/auth/src/api/providers/*.rs` do not import `StorageAdapter`.
- Public OAuth implementation files under `packages/functions/auth/src/oauth/*.rs` do not import `StorageAdapter`, excluding pure helper modules such as `pkce.rs`.
- Public provider domain files under `packages/functions/auth/src/providers/*.rs` do not import `StorageAdapter`.
- Public handler signatures do not contain `<S: StorageAdapter>`.
- `packages/functions/auth/src/lib.rs` does not re-export `StorageAdapter`.

Allowed raw storage areas:

- `packages/functions/auth/src/store/**`
- `packages/functions/auth/src/storage/**`
- `packages/functions/auth/tests/support/**`
- integration tests when they intentionally inspect test backend state
- `packages/functions/auth/src/api/admin.rs` only if admin migration is deferred in this slice

## Out Of Scope

- Removing the in-memory test backend.
- Adding DynamoDB Local or testcontainers.
- Changing DynamoDB table shape.
- Replacing the single-table design.
- Rewriting every internal `scan` into `Query`.
- Changing auth behavior or token claims.
- Changing provider flows.
- Changing admin lifecycle behavior unless the state migration is mechanical.
- AWS live deployment testing.
- Broad Rust formatting outside touched files.

## Expected Code Shape

Likely modified files:

```text
packages/functions/auth/src/config.rs
packages/functions/auth/src/main.rs
packages/functions/auth/src/lib.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/api/oauth/*.rs
packages/functions/auth/src/api/providers/*.rs
packages/functions/auth/src/oauth/*.rs
packages/functions/auth/src/providers/*.rs
packages/functions/auth/src/audit.rs
packages/functions/auth/src/ratelimit/middleware.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/rate_limits.rs
packages/functions/auth/src/storage/mod.rs
packages/functions/auth/src/storage/adapter.rs
packages/functions/auth/tests/support/mod.rs
packages/functions/auth/tests/*.rs
scripts/validate-store-boundary.mjs
package.json
```

Do not create a parallel storage tree unless it clearly reduces confusion. A small visibility cleanup is better than a broad file move in this slice.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add `scripts/validate-store-boundary.mjs` and wire it into `npm run test:infra`.
2. Run `npm run test:infra` and confirm the new validator fails on current `StorageAdapter`/`.storage` public handler usage.
3. Convert `AuthStore<S>` to non-generic `AuthStore` with an internal `Arc<dyn StorageAdapter>`.
4. Update store module impl blocks from `impl<S> AuthStore<S>` to `impl AuthStore`.
5. Add typed store methods for rate limiting and audit event recording so public handlers do not pass raw storage around.
6. Change public `AppState` to hold `store: AuthStore` and remove `storage`.
7. Update `main.rs` to construct `AuthStore` from `DynamoStorage`.
8. Update `routes.rs` to accept non-generic `AppState` and remove handler turbofish usages.
9. Update public OAuth handlers and provider handlers to use `state.store`.
10. Update provider domain functions to accept `&AuthStore` without storage generics.
11. Update public auth integration-test state builders for the new `AppState`.
12. Replace test raw-storage assertions with test-support helpers where needed.
13. Remove public `StorageAdapter` re-export from `lib.rs`.
14. Re-run `npm run test:infra`; it should pass the store-boundary validator.
15. Run focused Rust tests for affected domains:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test api_routes
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oauth_token_userinfo
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oauth_refresh_revoke
cargo test --manifest-path packages/functions/auth/Cargo.toml --test password_login
cargo test --manifest-path packages/functions/auth/Cargo.toml --test password_registration
cargo test --manifest-path packages/functions/auth/Cargo.toml --test password_reset
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_google_start
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_google_callback
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_apple_start
cargo test --manifest-path packages/functions/auth/Cargo.toml --test oidc_apple_callback
```

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

- `npm run test:infra` fails before the migration because public handlers import `StorageAdapter`.
- `npm run test:infra` fails before the migration because `routes.rs` reads `.storage`.
- `npm run test:infra` passes after public auth route/provider code uses `state.store`.
- The validator allows store internals and test support to use raw backend operations.
- The validator allows `api/admin.rs` only if admin migration is intentionally deferred.

### Rust Tests

Existing behavior tests must still pass:

- Public bootstrap route remains absent.
- Runtime client management routes remain absent.
- Password registration and verification still do not issue tokens.
- Password login still issues only authorization codes.
- Authorization-code exchange still validates PKCE and signs tokens with the runtime signer.
- Refresh rotation and `/oauth/revoke` still work.
- Google and Apple OIDC flows still issue internal authorization codes.
- Rate-limit tests still prove raw secrets do not appear in rate-limit keys.
- Key-shape tests still prove raw bearer values do not appear in stored keys.

No new auth behavior should be added in this slice.

## Acceptance Criteria

- Public `AppState` is non-generic and holds `store: AuthStore`.
- Public auth router construction is non-generic.
- Public OAuth and provider handlers do not import `StorageAdapter`.
- Public OAuth and provider handlers do not call `.storage`.
- Public handler signatures do not expose `<S: StorageAdapter>`.
- `AuthStore` is the only public auth runtime storage interface used by handlers.
- Production startup constructs `AuthStore` from `DynamoStorage`.
- Tests construct `AuthStore` from a simple in-memory backend.
- `StorageAdapter` is not re-exported as a top-level public runtime API.
- Static validation prevents public raw-storage access from returning.
- Existing auth behavior and security regression coverage remain intact.

## Manual Validation

No AWS validation is required for this slice. It is a Rust boundary cleanup.

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

After this slice, define `21_admin_store_boundary_or_internal_scan_cleanup`.

If `api/admin.rs` was deferred, the next slice should move IAM admin lifecycle routes behind the same `AuthStore` boundary.

If admin was migrated in this slice, the next slice should focus on remaining internal storage cleanup, such as making `StorageAdapter` crate-private, clarifying `storage/test_support.rs`, and replacing any remaining runtime scans with bounded query-shaped typed store operations where the design requires it.
