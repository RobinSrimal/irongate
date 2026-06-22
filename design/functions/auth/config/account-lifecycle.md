# Account Lifecycle Configuration

Target code: `packages/functions/auth/src/config/account_lifecycle.rs`

## Owns

- Deleted identity reuse policy.
- Deleted identity retention duration.
- Startup validation for account lifecycle settings.

## Does Not Own

- Account deletion shape.
- Which secret-bearing records are removed during deletion.
- Whether deletion is hard-delete or tombstone-based.

Deletion behavior is fixed by the account lifecycle core. Configuration only controls whether and when a deleted external identity may be reused for a new account.

## Runtime Config

```text
AUTH_DELETED_IDENTITY_REUSE optional, default after_retention
AUTH_DELETED_IDENTITY_RETENTION_DAYS optional, default 30
```

Supported `AUTH_DELETED_IDENTITY_REUSE` values:

```text
after_retention
immediate
never
```

## Behavior

`after_retention` keeps a deleted identity tombstone until the configured retention window has elapsed. After that window, the same password email or Google/Apple provider identity may create a new account with a new subject.

`immediate` releases the deleted identity for reuse as soon as deletion completes. The next registration or provider sign-in may create a new account with a new subject.

`never` permanently blocks reuse of the deleted identity. The same password email or Google/Apple provider identity cannot create a new account.

In every mode, re-registration must generate a new subject. The old subject is never reused.

These settings do not change what `delete_user(subject)` removes. Delete always uses the fixed anonymized tombstone policy defined in `../core/account-lifecycle.md`.

## Validation Rules

- Unknown reuse modes fail startup.
- `AUTH_DELETED_IDENTITY_RETENTION_DAYS` must be a positive integer when reuse mode is `after_retention`.
- The retention value should have an upper bound so accidental extreme retention is caught at startup.
- `immediate` and `never` do not require the retention setting.

Suggested production bound:

```text
1 to 3650 days
```

## Security Invariants

- Deleted identity tombstones store HMAC lookup digests, not raw email addresses or raw provider subjects.
- Reuse after retention is an explicit lifecycle transition, not an accidental overwrite.
- Reuse always creates a new account subject.
- Deletion behavior is not configurable through these settings.
- Audit events record deletion and reuse without logging raw identity secrets.
