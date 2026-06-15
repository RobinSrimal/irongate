# 13_iam_admin_delete_tombstones

## Goal

Add irreversible account deletion to the IAM-protected admin lifecycle API.

At the end of this slice, an operator can call `POST /_admin/users/{subject}/delete` through the separate admin Lambda. The operation marks the account deleted, revokes refresh-token sessions for the subject, removes auth-owned password hash and contact metadata, converts linked identities into anonymized tombstones, applies the configured deleted-identity reuse policy, and keeps future login, refresh, and userinfo paths blocked for the old subject.

This slice should not broaden the admin API beyond account lifecycle. It should not add dashboards, hosted UI, OAuth client management, hard deletes, or generic control-plane behavior.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/admin.md`
- `design/auth/core/account-lifecycle.md`
- `design/auth/config/account-lifecycle.md`
- `design/auth/store/accounts.md`
- `design/auth/store/identities.md`
- `design/auth/store/password-users.md`
- `design/auth/store/password-secrets.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/store/records.md`
- `design/auth/store/keys.md`
- `design/auth/observability/audit.md`
- `design/auth/testing.md`
- `design/infra/api.md`
- `design/infra/auth-function.md`
- `design/infra/iam.md`
- `design/scope.md`

The important design constraint is that deletion is fixed lifecycle behavior, not a configurable privacy shortcut. Config only controls whether and when deleted identity attributes may be reused for a new account. Delete itself must always strip auth-owned secret and contact material.

## Why This Slice Next

Slice 12 added the separate IAM-protected admin Lambda, sanitized account read, account disable, and subject refresh-token revocation. It intentionally stopped before deletion because deletion needs a wider data-shape change:

```text
operator IAM principal -> admin Lambda -> bounded subject indexes -> anonymized tombstones
```

Current account, identity, and password records are not enough to delete all auth-owned state by subject without either scanning the table or retaining too much data in deleted records. This slice fills that gap before AWS hardening and legacy removal.

## In Scope

### Admin Delete Route

Add:

```text
POST /_admin/users/{subject}/delete
```

Route requirements:

- Use API Gateway IAM auth through SST, matching the existing admin route shape.
- Route to the separate admin Lambda, not the public auth Lambda.
- Keep the Lambda-side IAM request-context guard.
- Reject missing or invalid IAM context with the existing admin forbidden response shape.
- Do not accept custom admin API keys.
- Return a small sanitized mutation response.

Suggested response shape:

```json
{
  "subject": "user_...",
  "status": "deleted",
  "deleted_at": "...",
  "revoked_refresh_families": 2,
  "deleted_identities": 1,
  "deleted_password_users": 1,
  "deleted_password_secrets": 0
}
```

The response may use slightly different count names if the implementation reads better, but it must not expose raw email addresses, password hashes, identity properties, provider profile claims, reset links, verification links, refresh tokens, authorization codes, signing keys, or raw DynamoDB values.

### Account Tombstone

Add a typed lifecycle operation:

```text
delete_account(subject, deleted_identity_reuse_policy, retention_days)
```

or an equivalent store/core orchestration with the same semantics.

Account deletion behavior:

- Unknown subject returns not found.
- Active and disabled accounts can be deleted.
- Repeated delete on an already deleted account is idempotent and returns the deleted account state.
- Deleted accounts cannot be restored by this slice.
- The old subject is never reused.
- `require_active_account` and equivalent checks continue to reject deleted accounts.

Target tombstone shape from the account lifecycle design:

```json
{
  "subject": "user_...",
  "status": "deleted",
  "deleted_at": "..."
}
```

If the current `AccountRecord` still requires non-secret operational fields such as `created_at` during this slice, keeping them temporarily is acceptable only when tests prove the tombstone contains no contact metadata, profile data, password material, token material, or raw identity data. Do not add new user-identifying fields to the account tombstone.

### Subject Indexes For Bounded Deletion

Delete must not scan the auth table.

Add or complete subject-index records for auth-owned state that must be found from a subject:

```text
identity_by_subject(subject, provider, identity_digest)
password_user_by_subject(subject, email_digest)
password_reset_by_subject(subject, reset_digest)
```

Rules:

- Index records store HMAC lookup digests, not raw email addresses or raw provider subjects.
- Index records do not store password hashes, profile claims, reset tokens, verification tokens, refresh tokens, or raw OAuth artifacts.
- Account creation, password verification, Google callback, and Apple callback paths write the identity subject index for new active identities.
- Password verification writes the password-user subject index when an unverified password user becomes a verified account.
- Password reset creation writes the reset-secret subject index so deletion can remove active reset secrets without a scan.
- Existing pre-production records without these indexes do not need a migration in this slice.

The subject indexes are implementation support for lifecycle operations, not public APIs.

### Identity Tombstones

When deleting a subject, every active identity linked through `identity_by_subject:<subject>` must be converted into a deleted tombstone.

Deleted identity tombstone target:

```json
{
  "provider": "password|google|apple",
  "identity_digest": "...",
  "status": "deleted",
  "deleted_at": "...",
  "reuse_after": "optional"
}
```

Rules:

- Remove the old subject from the deleted identity record.
- Remove provider profile properties and claims.
- Remove raw email or contact data.
- Keep only HMAC lookup material and deletion/reuse metadata.
- Delete or tombstone the matching `identity_by_subject` index record so later admin reads by old subject do not expose identity details.
- Reuse creates a new subject and a fresh active identity record; it must not resurrect the old subject.

Deleted identity reuse policy:

```text
AUTH_DELETED_IDENTITY_REUSE=after_retention
AUTH_DELETED_IDENTITY_REUSE=immediate
AUTH_DELETED_IDENTITY_REUSE=never
```

Behavior:

- `after_retention` sets `reuse_after = deleted_at + AUTH_DELETED_IDENTITY_RETENTION_DAYS`.
- `immediate` sets `reuse_after = deleted_at` or an equivalent immediately reusable marker.
- `never` leaves no reusable timestamp and blocks reuse.

The implementation should adjust `reuse_deleted_identity` so it no longer depends on old-subject or old-profile data inside the deleted identity tombstone.

### Password User Tombstones

When deleting a subject, every verified password user linked through `password_user_by_subject:<subject>` must remove auth-owned credential and contact material.

Target behavior:

- Remove or null out `password_hash`.
- Remove or null out raw `email`.
- Mark the password user deleted or replace it with a password-user tombstone.
- Preserve only HMAC lookup material needed to enforce deleted identity reuse policy and prevent accidental immediate recreation when policy blocks it.
- Remove the `password_user_by_subject` index after the password user is tombstoned.
- Future password login and password reset for the deleted subject must fail.

The current `PasswordUserRecord` uses required `email` and `password_hash` fields. This slice should replace that shape with an enum or optional/tombstone-friendly record structure instead of storing empty fake credentials.

### Password Secret Cleanup

Delete active password reset secrets for the subject by using subject-index records.

Required operation:

```text
delete_password_secrets_for_subject(subject)
```

Rules:

- Delete reset records and their `password_reset_by_subject` index records for the deleted subject.
- Do not scan `password:reset`.
- Do not log raw reset tokens or reset digests.
- Email verification records that are not linked to a subject should remain TTL-governed; do not scan for them.

This keeps deletion bounded while still removing subject-owned reset secrets that would otherwise allow credential recovery after deletion.

### Refresh Session Revocation

Deletion must revoke all refresh-token families for the subject using the subject index introduced before this slice.

Rules:

- Use `revoke_refresh_tokens_for_subject`.
- Do not revoke already-issued access JWTs or ID tokens because they are self-contained and expire naturally.
- Keep the operation idempotent.
- Do not scan all refresh tokens.

### Audit Event

Emit a sanitized audit event for successful delete where the current audit layer supports it:

```text
admin_account_deleted
```

Audit detail may include counts, such as:

```text
revoked_refresh_families=2 deleted_identities=1 deleted_password_users=1 deleted_password_secrets=0
```

Audit events must not include raw email addresses, password hashes, identity profile properties, reset links, verification links, provider tokens, refresh tokens, authorization codes, provider state, signing keys, or raw DynamoDB records.

## Out Of Scope

- Account re-enable route.
- Account restore or undelete.
- Hard deletion with no tombstone.
- Runtime OAuth client management.
- Public admin bootstrap.
- Custom admin API keys.
- Hosted or local admin UI.
- Dashboard or reporting features.
- Deleting application-owned business data outside the auth table.
- Production data migration for old pre-slice records.
- Google or Apple provider feature changes beyond writing subject indexes.
- API Gateway source-IP hardening.
- Optional customer managed KMS.
- KMS ES256 signing implementation.
- Live AWS validation beyond static SST/type checks.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree where practical.

Target modules:

```text
infra/api.ts
packages/functions/auth/src/api/admin.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/password_users.rs
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/store/refresh.rs
packages/functions/auth/tests/admin_deletion_slice.rs
packages/functions/auth/tests/admin_lifecycle_slice.rs
```

If splitting account and identity store operations into `store/accounts.rs` and `store/identities.rs` is small and reduces coupling, it is acceptable. Do not turn this slice into a broad folder-realignment project if the deletion behavior can be implemented safely in the current store layout.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing store tests for account deletion from active, disabled, already deleted, and unknown account states.
2. Add failing store tests for identity subject-index creation during password, Google, and Apple account creation.
3. Add failing store tests for identity tombstones that remove old subject and properties while preserving provider, identity digest, status, deletion time, and reuse metadata.
4. Add failing store tests for deleted identity reuse under `after_retention`, `immediate`, and `never`.
5. Add failing store tests for password-user subject-index creation during password verification.
6. Add failing store tests for password-user tombstones that remove raw email and password hash material.
7. Add failing store tests for password reset subject-index creation and subject deletion cleanup.
8. Implement subject-index key helpers and record shapes.
9. Update account/identity/password verification and provider callback paths to write the required subject indexes.
10. Implement password reset subject indexes and `delete_password_secrets_for_subject`.
11. Implement the central delete-account orchestration using bounded queries and transactions.
12. Add route tests for `POST /_admin/users/{subject}/delete`.
13. Wire the admin delete handler and sanitized mutation response.
14. Add the SST route `POST /_admin/users/{subject}/delete` with IAM auth and the admin Lambda integration.
15. Add tests proving delete blocks future refresh and password login for the old subject.
16. Update docs only if implementation decisions refine route response shape or record naming.
17. Run focused admin deletion tests, full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Store Tests

- Active account can be deleted.
- Disabled account can be deleted.
- Already deleted account deletion is idempotent.
- Unknown account deletion returns not found.
- Deleted account is not active.
- Deletion revokes every indexed refresh-token family for the subject.
- Deletion uses bounded subject-index queries, not full table scans.
- Account tombstone contains no contact metadata, password hash, provider claims, token material, or raw DynamoDB blob.
- Identity subject indexes are written for password, Google, and Apple identities.
- Identity tombstones remove old subject and provider properties.
- Deleted identity reuse creates a new subject.
- `after_retention` blocks reuse before retention and allows reuse after retention.
- `immediate` allows reuse after deletion.
- `never` blocks reuse after deletion.
- Password-user subject index is written when password verification creates the account subject.
- Password-user tombstone removes raw email and password hash material.
- Deleted password users cannot authenticate or reset password.
- Password reset subject indexes are written on reset creation.
- `delete_password_secrets_for_subject` removes active reset records and index records.
- Subject deletion does not scan email verification records that have no subject.

### Route Tests

- `POST /_admin/users/{subject}/delete` requires IAM request-context evidence.
- `POST /_admin/users/{subject}/delete` rejects custom admin API keys.
- `POST /_admin/users/{subject}/delete` marks the account deleted.
- `POST /_admin/users/{subject}/delete` returns a sanitized response.
- Delete response does not contain email, password hash, identity properties, refresh tokens, reset tokens, verification tokens, signing keys, or raw DynamoDB values.
- Repeated delete returns a successful deleted state without restoring anything.
- `GET /_admin/users/{subject}` after delete returns sanitized deleted account status.
- Refresh-token grant after delete fails.
- Password login after delete fails without issuing an authorization code.
- Userinfo for a deleted account fails when account-status checks are applied.

### Infra Tests

- `POST /_admin/users/{subject}/delete` points to the admin Lambda.
- `POST /_admin/users/{subject}/delete` has IAM auth.
- Public `$default` still points to the public auth Lambda.
- Admin Lambda environment includes only lifecycle-required settings.

If SST's constructs make direct automated route-auth assertions awkward, use the strongest available static/typecheck coverage and document manual AWS validation steps.

## Acceptance Criteria

- Operators can delete an account through an IAM-protected admin route.
- Delete runs in the separate admin Lambda.
- Delete is irreversible for the old subject.
- Delete is idempotent for already deleted accounts.
- Deleted accounts cannot log in, refresh, receive userinfo, or be restored by this slice.
- Deletion revokes all refresh-token families for the subject.
- Deletion removes password hashes and raw email contact data from auth-owned password records.
- Deletion converts linked identities into anonymized tombstones.
- Deleted identity tombstones contain no old subject, provider profile claims, or raw identity values.
- Deleted identity reuse follows `AUTH_DELETED_IDENTITY_REUSE` and `AUTH_DELETED_IDENTITY_RETENTION_DAYS`.
- All deletion lookups use exact keys, bounded subject-index queries, or transactions.
- No deletion path scans the full auth table.
- Admin responses and audit events do not include raw secrets, tokens, password hashes, emails, provider claims, or raw DynamoDB records.
- Public password, Google, Apple, token, refresh, revoke, and userinfo flows continue to pass.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_deletion_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml --test admin_lifecycle_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/admin/Cargo.toml
npm run typecheck
npm run test:setup
```

AWS validation should be done before production confidence:

```text
unsigned request to POST /_admin/users/{subject}/delete -> rejected by API Gateway
SigV4 request with allowed IAM principal -> reaches admin Lambda
SigV4 request without route permission -> rejected by API Gateway/IAM
public OAuth route -> still does not require IAM
deleted subject refresh token -> rejected
deleted subject password login -> rejected
```

## Next Slice

After this slice, implement `14_aws_hardening_and_runtime_validation`.

That slice should tighten AWS-specific deployment behavior, including API Gateway request-context source IP for rate limits, least-privilege IAM, CloudWatch audit logging defaults and opt-out, optional customer managed DynamoDB KMS, KMS ES256 signing mode, and AWS dev deployment smoke tests.
