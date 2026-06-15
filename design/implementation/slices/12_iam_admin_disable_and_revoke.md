# 12_iam_admin_disable_and_revoke

## Goal

Add the first operator-only account lifecycle boundary using a separate admin Lambda.

At the end of this slice, SST should deploy the public auth Lambda for public OAuth/provider/password routes and a separate admin Lambda for IAM-protected `/_admin/*` routes. Operators should be able to read a sanitized account status, disable an account, and revoke all refresh-token sessions for a subject without exposing custom admin API keys or mounting lifecycle code behind the public `$default` route.

This slice intentionally stops before irreversible account deletion. Deletion requires fixed anonymized tombstones, password hash removal, contact/profile metadata stripping, identity tombstone handling, and deleted-identity reuse policy enforcement. That belongs in the next slice.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/admin.md`
- `design/auth/core/account-lifecycle.md`
- `design/auth/store/accounts.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/store/records.md`
- `design/auth/store/keys.md`
- `design/auth/observability/audit.md`
- `design/auth/testing.md`
- `design/infra/api.md`
- `design/infra/auth-function.md`
- `design/infra/iam.md`
- `design/scope.md`

The important design constraint is that admin lifecycle is control-plane, not public auth runtime. This slice must not reintroduce public `/admin/bootstrap`, custom admin API keys, runtime OAuth client management, hosted operator UI assumptions, or raw DynamoDB record exposure.

## Why This Slice Next

Password, refresh-token, Google, and Apple login flows now create durable account and identity state. The next missing foundation is controlled operator access for lifecycle operations:

```text
operator IAM principal -> API Gateway IAM route -> admin Lambda -> typed lifecycle store operation
```

Separating the admin Lambda now creates a cleaner boundary before adding irreversible deletion behavior.

## In Scope

### Separate Admin Lambda

Add a second Rust Lambda entrypoint for admin lifecycle routes.

Target code:

```text
packages/functions/auth/src/admin_main.rs or equivalent binary entrypoint
packages/functions/auth/src/api/admin.rs
packages/functions/auth/src/routes.rs or a separate admin router module
infra/api.ts
```

Required behavior:

- Public `$default` continues to route to the public auth Lambda.
- `/_admin/*` routes invoke the admin Lambda, not the public auth Lambda.
- Admin Lambda reuses shared store/core/audit modules.
- Admin Lambda does not load or require Resend secrets, Google client secret, Apple private key, or local JWT signing private key unless a future lifecycle route proves it needs them.
- Admin Lambda receives only runtime settings needed for lifecycle operations.

Suggested initial admin runtime dependencies:

```text
DYNAMODB_TABLE
AUTH_HMAC_LOOKUP_SECRET if needed for indexed lifecycle cleanup
AUTH_AUDIT_LOG_MODE
AUTH_DELETED_IDENTITY_REUSE only when deletion lands later
AUTH_DELETED_IDENTITY_RETENTION_DAYS only when deletion lands later
```

If the implementation path shares `RuntimeAuthConfig` initially, it must document the temporary over-broad config and keep route/permission isolation intact. The preferred target is a smaller admin config.

### API Gateway IAM Routes

Add explicit admin routes with IAM authorization:

```text
GET  /_admin/users/{subject}
POST /_admin/users/{subject}/disable
POST /_admin/users/{subject}/revoke-sessions
```

Route requirements:

- Use API Gateway IAM auth, for example `auth: { iam: true }`.
- Route to the admin Lambda.
- Do not rely on CORS, cookies, bearer tokens, or custom admin headers.
- Keep public auth routes unauthenticated by IAM.
- Add operator IAM policy examples or route ARN outputs if useful for template users.

The Lambda should still reject admin requests if expected API Gateway/IAM request-context evidence is missing. This is a defense-in-depth check, not the primary authorization layer.

### Sanitized Account Read

Add:

```text
GET /_admin/users/{subject}
```

Response shape should be sanitized and small:

```json
{
  "subject": "user_...",
  "status": "active|disabled|deleted",
  "created_at": "...",
  "disabled_at": "optional",
  "deleted_at": "optional"
}
```

Rules:

- Do not return password hashes.
- Do not return raw email addresses from password records.
- Do not return identity properties/profile claims.
- Do not return refresh-token records.
- Do not return authorization codes, provider states, verification tokens, reset tokens, signing keys, or raw DynamoDB JSON.

### Disable Account

Add:

```text
POST /_admin/users/{subject}/disable
```

Store behavior:

- Add `disabled` to the account status model.
- Mark the account disabled with a `disabled_at` timestamp.
- Operation is idempotent for already disabled accounts.
- Deleted accounts cannot be re-disabled or restored by this route.
- Disable must revoke all refresh-token families for the subject.
- Future password, Google, Apple, authorization-code issuance, refresh rotation, and userinfo paths must reject disabled accounts through the existing active-account checks.

Suggested account record target shape after this slice:

```json
{
  "subject": "user_...",
  "status": "active|disabled|deleted",
  "created_at": "...",
  "disabled_at": "optional",
  "deleted_at": "optional"
}
```

If the current code needs a compatibility migration for older account records without `disabled_at`, this repo is still pre-production, so tests may update fixtures directly rather than adding a data migration layer.

### Revoke Sessions

Add:

```text
POST /_admin/users/{subject}/revoke-sessions
```

Store behavior:

- Revoke all active refresh-token families for the subject using `refresh_by_subject` index records.
- Do not scan the full auth table.
- Do not revoke already-issued access JWTs or ID tokens because those are self-contained and expire naturally.
- Operation is idempotent if the subject has no refresh-token sessions.
- Operation should work for active, disabled, and deleted account subjects as long as the account record exists.

This route is also used by `disable` internally, but exposing it separately gives operators a less destructive logout-all-sessions action.

### Audit Events

Emit sanitized audit events where the current audit layer supports it:

- `admin_account_read`
- `admin_account_disabled`
- `admin_subject_sessions_revoked`
- `admin_lifecycle_failed`

Audit events must not include raw tokens, password hashes, email addresses, reset/verification links, provider state, provider tokens, signing keys, or raw DynamoDB records.

If the current audit module shape makes full event wiring too broad, add a focused follow-up note and keep this slice from logging secrets.

## Out Of Scope

- `POST /_admin/users/{subject}/delete`.
- Password hash removal.
- Password-user metadata stripping.
- Identity tombstone anonymization.
- Deleted identity reuse transitions.
- Account re-enable route.
- OAuth client management.
- Runtime client CRUD.
- Public admin bootstrap.
- Custom admin API keys.
- Hosted or local admin UI.
- API Gateway source-IP hardening for public rate limits.
- KMS signing implementation.
- Live AWS IAM validation beyond static SST/type checks.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree.

Target modules:

```text
infra/api.ts
packages/functions/auth/src/admin_main.rs or package-supported binary entrypoint
packages/functions/auth/src/api/admin.rs
packages/functions/auth/src/api/mod.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/refresh.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/tests/admin_lifecycle_slice.rs
```

Legacy runtime admin-key modules may remain compiled until the legacy-removal slice, but the target admin routes must not use:

```text
packages/functions/auth/src/admin/auth.rs public bootstrap
custom x-admin-key style auth
runtime OAuth client CRUD
```

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing store tests for `disabled` account status, `disable_account`, idempotent disable, and deleted-account disable rejection.
2. Update account records/status and implement typed disable/read helpers.
3. Add failing refresh-store tests for `revoke_refresh_tokens_for_subject` using subject index records without table scans.
4. Implement subject refresh revocation.
5. Add failing route tests for sanitized account read, disable, and revoke-sessions.
6. Implement admin API handlers and an admin router.
7. Add failing route tests proving no custom admin API key is accepted and raw secret-bearing records are not returned.
8. Add failing infra/type tests or static assertions for explicit `/_admin/*` routes with IAM auth and separate admin Lambda binding.
9. Update SST route wiring to use a separate admin Lambda.
10. Update docs if implementation decisions refine admin config, route names, or IAM policy examples.
11. Run focused admin lifecycle tests, full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Store Tests

- Active account can be disabled.
- Disabled account remains disabled on repeated disable.
- Deleted account cannot be disabled or restored.
- `is_active_account` returns false for disabled and deleted accounts.
- Disable response does not expose contact metadata.
- Subject refresh-token revocation uses `oauth:refresh_by_subject:<subject>` index records.
- Subject refresh-token revocation marks every active family revoked.
- Subject refresh-token revocation is idempotent when there are no sessions.
- Subject refresh-token revocation does not require a table scan.

### Route Tests

- `GET /_admin/users/{subject}` returns sanitized account status.
- `GET /_admin/users/{subject}` does not return password hash, raw email, identity properties, refresh-token records, or raw DynamoDB values.
- `POST /_admin/users/{subject}/disable` marks the account disabled.
- `POST /_admin/users/{subject}/disable` revokes refresh-token families for that subject.
- Disabled subjects cannot refresh tokens.
- Disabled subjects cannot call `/userinfo` with newly issued tokens after disable checks are applied.
- `POST /_admin/users/{subject}/revoke-sessions` revokes refresh-token families without disabling the account.
- Admin routes reject requests when expected API Gateway/IAM request-context evidence is absent.
- Custom admin API keys do not authenticate target admin routes.

### Infra Tests

- Public `$default` route points to public auth Lambda.
- `GET /_admin/users/{subject}` points to admin Lambda and has IAM auth.
- `POST /_admin/users/{subject}/disable` points to admin Lambda and has IAM auth.
- `POST /_admin/users/{subject}/revoke-sessions` points to admin Lambda and has IAM auth.
- Admin Lambda environment omits provider/email/signing secrets by default.

If SST's constructs make direct automated route-auth assertions awkward, use the strongest available static/typecheck coverage and document manual AWS validation steps.

## Acceptance Criteria

- Admin lifecycle is served by a separate admin Lambda.
- Public auth Lambda does not mount target `/_admin/*` lifecycle routes behind `$default`.
- API Gateway requires IAM on the implemented admin routes.
- Admin Lambda has a defense-in-depth request-context guard.
- Operators can read sanitized account status.
- Operators can disable an account.
- Operators can revoke all refresh-token sessions for a subject.
- Disabled accounts cannot receive new authorization codes, refresh tokens, or userinfo responses.
- Refresh-token revocation by subject uses bounded subject index records, not table scans.
- Admin responses and audit events do not include raw secrets, tokens, password hashes, emails, provider claims, or raw DynamoDB records.
- Public password, Google, Apple, token, refresh, revoke, and userinfo flows continue to pass.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_lifecycle_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

AWS validation should be done before production confidence:

```text
unsigned request to /_admin/users/{subject} -> rejected by API Gateway
SigV4 request with allowed IAM principal -> reaches admin Lambda
SigV4 request without route permission -> rejected by API Gateway/IAM
public OAuth route -> still does not require IAM
```

## Next Slice

After this slice, implement `13_iam_admin_delete_tombstones`.

That slice should add irreversible deletion with fixed anonymized tombstones, password hash removal, identity metadata stripping, deleted identity reuse policy enforcement, and session revocation.
