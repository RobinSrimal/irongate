# Auth Store

Target code: `packages/functions/auth/src/store`

## Owns

- Concrete DynamoDB persistence for auth.
- Typed record serialization.
- Typed key construction.
- Conditional writes and transactions.
- TTL handling.

## Must Not Own

- HTTP request parsing.
- Provider identity validation.
- Token signing internals.

## Target Direction

Use one concrete DynamoDB store instead of a generic storage abstraction.

The store should expose domain operations:

```text
create_authorize_session
take_authorize_session
create_authorization_code
take_authorization_code
create_password_user
verify_password_login
create_email_verification
consume_email_verification
create_password_reset
consume_password_reset
delete_password_secrets_for_subject
create_account
get_account
require_active_account
disable_account
mark_account_deleted
get_identity
create_identity_from_verified_proof
touch_identity_last_seen
mark_identity_deleted
reuse_deleted_identity_with_new_subject
rotate_refresh_token
revoke_refresh_token_family
revoke_refresh_tokens_for_subject
check_rate_limit
```

One-time records should use `take` or `consume` methods so replay behavior is visible in the type-level API.

The target store should avoid a generic `one_time` module. Authorization codes, provider callback state, and password verification/reset secrets have different validation rules and should live in explicit modules.

## Design Files

- `authorization-codes.md`
- `accounts.md`
- `authorize-sessions.md`
- `dynamodb.md`
- `identities.md`
- `keys.md`
- `password-secrets.md`
- `password-users.md`
- `provider-states.md`
- `rate-limits.md`
- `records.md`
- `refresh-tokens.md`
