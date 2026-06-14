# Account Lifecycle

Target code: `packages/functions/auth/src/core/account_lifecycle.rs`

## Owns

- Account status model.
- Disable, delete, and session revocation rules.
- Rules for issuing tokens to active accounts only.

## Account Status

Every subject has an account record with a lifecycle status:

```text
active
disabled
deleted
```

Token issuance must check account status before issuing an authorization code, access token, ID token, or refresh token. Refresh-token rotation must also check account status.

Access tokens and ID tokens are signed JWTs and are not persisted. Disabling or deleting an account cannot instantly revoke already-issued JWTs unless downstream APIs check account status on every request. This is why access-token and ID-token TTLs should stay short.

## Disable User

`disable_user(subject)` is a reversible operator action.

It should:

- Mark the account `disabled`.
- Revoke refresh tokens for the subject.
- Delete pending verification and reset secrets for the subject where possible.
- Prevent future login, authorization-code issuance, token refresh, and userinfo responses.
- Preserve identity mappings and contact metadata so the account can be reviewed or re-enabled later.

## Delete User

`delete_user(subject)` is irreversible account removal from the auth system.

Delete behavior is not config-based. The target core uses one fixed deletion policy: anonymized tombstones.

It must:

- Mark the account `deleted`.
- Revoke refresh tokens for the subject.
- Delete pending reset secrets and any verification secrets tied to the subject.
- Remove email, profile, and other contact metadata from password and identity records.
- Remove password hash material.
- Prevent future login, authorization-code issuance, token refresh, and userinfo responses for that subject.
- Keep only minimal tombstone metadata needed to prevent record resurrection and enforce identity reuse policy.

Fixed tombstone shape:

```text
account tombstone: subject, status=deleted, deleted_at
identity tombstone: provider, identity_digest, status=deleted, deleted_at, optional reuse_after
```

Deleted records must not retain:

```text
password hash
raw email
profile claims
provider claims
refresh tokens
verification or reset secrets
raw bearer values
```

Sanitized audit events are retained according to the audit logging policy. They must not contain raw identity secrets, passwords, tokens, or deleted contact metadata. If a product needs legal-grade erasure behavior beyond this fixed auth deletion policy, that requires a separate data retention design outside the auth core.

## Subject Reuse

Subjects should be generated and persisted, not deterministically re-derived from email or provider identity. This prevents a deleted account from re-registering and receiving the same `sub` by accident.

Whether the same email or provider identity may register again after deletion is config-based:

```text
AUTH_DELETED_IDENTITY_REUSE=after_retention | immediate | never
AUTH_DELETED_IDENTITY_RETENTION_DAYS=30
```

`after_retention` is the default. It keeps a deleted identity tombstone for the configured window, then allows reuse with a new subject. `immediate` allows reuse right away with a new subject. `never` permanently blocks that identity from registering again.

In every mode, the previous subject is never reused. Reuse is an explicit lifecycle transition that creates a new account and emits an audit event.

## Security Invariants

- Disabled or deleted accounts cannot receive new tokens.
- Refresh tokens are revoked when an account is disabled or deleted.
- Previously issued JWTs expire naturally according to configured TTLs.
- Account deletion does not expose raw auth records to operators.
- Account lifecycle operations are reachable only through IAM-protected admin routes.
- Delete behavior is fixed and not configuration-dependent.
- Deleted identity reuse follows configured policy and never reuses the old subject.
