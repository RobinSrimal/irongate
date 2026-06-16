# Account Store

Target code: `packages/functions/auth/src/store/accounts.rs`

## Owns

- Persisted account records keyed by subject.
- Account status updates.
- Account deletion tombstones.

## Target Records

```text
account:<subject>
```

Value:

```json
{
  "subject": "user:...",
  "status": "active",
  "created_at": "...",
  "updated_at": "...",
  "disabled_at": "optional",
  "deleted_at": "optional"
}
```

Contact metadata belongs in identity/password records and is removed during deletion. The account record stays small and does not contain bearer secrets, password hashes, provider tokens, email addresses, profile claims, or email verification/reset links.

Deleted account records remain as subject tombstones. Identity reuse policy controls whether the same external identity may later point at a new subject; it never reactivates or reuses the deleted account subject.

Deleted account tombstone shape:

```json
{
  "subject": "user:...",
  "status": "deleted",
  "deleted_at": "..."
}
```

## Store Operations

```text
create_account
get_account
require_active_account
disable_account
enable_account
mark_account_deleted
```

`create_account` should generate a new opaque subject ID with secure randomness and create the account with a conditional write. Subject IDs should be stable after creation but not re-derived from provider claims.

## Security Invariants

- Account status changes are conditional and auditable.
- Disabled accounts can be enabled; deleted accounts cannot be enabled.
- Deleted accounts cannot be silently restored by a login path.
- Deleted account subjects are not reused, regardless of identity reuse policy.
- Deleted account tombstones do not contain contact metadata or secret-bearing fields.
- The account record is safe to read for lifecycle decisions, but still not a public profile record.
- Token issuance paths call `require_active_account` before creating tokens.
