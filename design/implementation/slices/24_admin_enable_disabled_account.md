# 24_admin_enable_disabled_account

## Goal

Add a narrow IAM-protected admin route that re-enables disabled accounts without weakening delete semantics.

At the end of this slice, an operator can call:

```text
POST /_admin/users/{subject}/enable
```

through the admin Lambda. The operation must support only:

```text
disabled -> active
active -> active
```

Deleted accounts remain terminal. This slice should make `disable` operationally reversible while keeping `delete` irreversible.

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
- `design/infra/operator-iam-policy.md`
- `design/scope.md`

The important design constraint is that this is account lifecycle only. It must not add undelete, OAuth client management, custom admin API keys, hosted admin UI, dashboards, or direct raw-table operator access.

## Why This Slice Next

The current admin lifecycle routes are:

```text
GET  /_admin/users/{subject}
POST /_admin/users/{subject}/disable
POST /_admin/users/{subject}/delete
POST /_admin/users/{subject}/revoke-sessions
```

The account lifecycle design describes disable as a reversible operator action, but the implementation has no route back to `active`. That makes `disable` awkward operationally: it is safer than delete, but still leaves no clean recovery path.

This slice fills that gap with one small route:

```text
active -> disabled -> active
active -> deleted
disabled -> deleted
deleted -> terminal
```

## In Scope

### Account Enable Store Operation

Add a typed store operation:

```text
enable_account(subject)
```

Required behavior:

- Unknown subject returns not found.
- Active account is idempotent and returns active.
- Disabled account becomes active.
- Deleted account returns a conflict/invalid lifecycle transition.
- Enabling clears `disabled_at` from the account record.
- Enabling never changes `deleted_at`.
- Enabling does not recreate deleted identities, password users, reset secrets, or verification secrets.
- Enabling does not read raw auth records outside typed store operations.

Suggested account transition:

```json
{
  "subject": "user_...",
  "status": "active",
  "created_at": "...",
  "disabled_at": null,
  "deleted_at": null
}
```

### Refresh Session Revocation On Enable

Enabling should revoke refresh-token families for the subject.

Reasoning:

- Disable already revokes refresh sessions.
- Enabling should not make any stale session state useful.
- The restored user should log in fresh.

Required behavior:

- Use `revoke_refresh_tokens_for_subject`.
- Do not scan the auth table.
- Return the number of revoked refresh-token families in the admin response.
- Already-issued access JWTs and ID tokens still expire naturally.

### Admin Enable Route

Add:

```text
POST /_admin/users/{subject}/enable
```

Route requirements:

- API Gateway IAM authorization through SST, matching the existing admin route shape.
- Route to the separate admin Lambda, not the public auth Lambda.
- Keep the Lambda-side IAM request-context guard.
- Reject unsigned requests and custom admin-key attempts.
- Return a sanitized mutation response.

Suggested response shape:

```json
{
  "subject": "user_...",
  "status": "active",
  "revoked_refresh_families": 1
}
```

The response may also include `created_at` if that matches the existing response helper, but it must not include password hashes, raw emails, identity properties, reset links, verification links, refresh tokens, authorization codes, signing keys, or raw DynamoDB values.

### Audit Event

Emit a sanitized audit event:

```text
admin_account_enabled
```

Audit detail may include:

```text
revoked_refresh_families=<count>
```

Audit events must not include raw emails, password hashes, identity profile properties, reset links, verification links, provider tokens, refresh tokens, authorization codes, provider state, signing keys, or raw DynamoDB records.

### Infra Route Wiring

Update SST route wiring:

```text
POST /_admin/users/{subject}/enable -> admin Lambda, IAM required
```

Update static infra validators so this route cannot accidentally land on the public Lambda or lose IAM auth.

Update admin route outputs/operator IAM documentation if needed. The existing broad admin ARN pattern may already cover this route, but the operator policy docs should mention the explicit path.

### Design Doc Updates

Update design docs to include enable behavior:

- `design/auth/api/admin.md`
- `design/auth/core/account-lifecycle.md`
- `design/infra/api.md`
- `design/infra/iam.md` or `design/infra/operator-iam-policy.md` if route examples list explicit actions.

## Out Of Scope

- Restoring deleted accounts.
- Recreating deleted identities.
- Recreating deleted password users.
- Reversing password/contact tombstones.
- Hard delete.
- Runtime OAuth client management.
- Public admin bootstrap.
- Custom admin API keys.
- Hosted or local admin UI.
- Dashboard/reporting features.
- Application-owned business data lifecycle.
- New account status values.
- Production data migration.
- Changing access-token or ID-token revocation semantics.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree where practical.

Target modules:

```text
infra/api.ts
scripts/validate-infra-routes.mjs
scripts/validate-infra-hardening.mjs if route assertions live there
packages/functions/auth/src/api/admin.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/store/refresh.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/tests/admin_lifecycle.rs
packages/functions/auth/tests/admin_deletion.rs if deleted-transition coverage belongs there
design/auth/api/admin.md
design/auth/core/account-lifecycle.md
design/infra/api.md
design/infra/iam.md
```

Do not create a new generic admin framework for this route. Extend the existing admin router and typed store lifecycle operations.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add store tests for `enable_account`.
2. Implement `enable_account` in the typed store.
3. Add tests that deleted accounts cannot be enabled.
4. Add tests that enabling revokes refresh-token families for the subject.
5. Add admin route tests for `POST /_admin/users/{subject}/enable`.
6. Add tests for missing IAM context and custom admin-key rejection.
7. Wire the route in `api/admin.rs`.
8. Wire the route in `infra/api.ts` with IAM authorization.
9. Update static infra validation for the new route.
10. Update design docs and operator IAM route examples.
11. Run focused admin lifecycle tests.
12. Run full Rust tests, `cargo check`, `npm run typecheck`, and `npm run test:infra`.
13. Optionally deploy to dev and smoke test active/disabled/deleted transitions with a disposable account.

## Tests

### Store Tests

- Active account enable is idempotent.
- Disabled account becomes active.
- Enabled account has `disabled_at = None`.
- Deleted account cannot be enabled.
- Unknown account returns not found.
- Enabling revokes refresh-token families for the subject.
- Enabling does not scan the auth table.

### Route Tests

- `POST /_admin/users/{subject}/enable` returns active for a disabled account.
- Repeated enable on an active account returns active.
- Enable on a deleted account returns conflict.
- Enable on an unknown subject returns not found.
- Enable response is sanitized.
- Enable emits `admin_account_enabled` where audit capture is available.
- Missing IAM request context is forbidden.
- `x-admin-key` does not authorize the route.

### Infra Tests

- SST defines `POST /_admin/users/{subject}/enable`.
- The route points to the admin Lambda.
- The route has IAM auth enabled.
- Public `$default` still points to the public auth Lambda.
- Admin Lambda remains separate from the public auth Lambda.

## Acceptance Criteria

- Disabled accounts can be re-enabled through an IAM-protected admin route.
- Active account enable is idempotent.
- Deleted accounts remain terminal and cannot be enabled.
- Enabling revokes refresh-token families and forces fresh login.
- No raw auth secrets or raw DynamoDB records are returned.
- No custom admin API keys are accepted.
- Route wiring keeps admin traffic on the admin Lambda with API Gateway IAM auth.
- Design docs describe the lifecycle transition clearly.
- Full local verification passes.

## Manual Validation

Dev smoke test with a disposable account:

```text
register + verify disposable password user
login and receive refresh token
POST /_admin/users/{subject}/disable with SigV4
confirm login/refresh fail while disabled
POST /_admin/users/{subject}/enable with SigV4
confirm old refresh token remains unusable
confirm fresh login succeeds
confirm GET /_admin/users/{subject} returns active
```

Do not use a long-lived personal account for destructive delete validation. This slice does not require deleting any account.

## Next Slice

After this slice, choose the next step based on remaining AWS smoke gaps.

Likely follow-ups:

- Update AWS dev smoke checklist to include disable/enable lifecycle validation.
- Add a small CLI smoke helper if manual lifecycle testing becomes repetitive.
- Consider production-stage validation once dev lifecycle behavior is stable.
