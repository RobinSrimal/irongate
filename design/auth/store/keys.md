# Store Keys

Target code: `packages/functions/auth/src/store/keys.rs`

## Owns

- DynamoDB `pk` and `sk` construction.
- HMAC lookup key construction.
- Key naming conventions.

## Target Behavior

Keys should be typed helpers, not ad hoc string arrays.

Examples:

```text
client_config(client_id)
authorize_session(session_lookup_digest)
provider_state(provider_state_lookup_digest)
auth_code(auth_code_lookup_digest)
refresh_token(refresh_lookup_digest)
refresh_by_subject(subject, refresh_lookup_digest)
refresh_by_client(client_id, refresh_lookup_digest)
password_user(email_digest)
email_verification(verification_lookup_digest)
password_reset(reset_lookup_digest)
```

## HMAC Key Families

Bearer-style secrets must be converted to deterministic lookup digests before they become DynamoDB keys:

```text
session_lookup_digest = HMAC-SHA256(storage_lookup_secret, session_key)
provider_state_lookup_digest = HMAC-SHA256(storage_lookup_secret, provider_state)
auth_code_lookup_digest = HMAC-SHA256(storage_lookup_secret, authorization_code)
refresh_lookup_digest = HMAC-SHA256(storage_lookup_secret, refresh_token)
verification_lookup_digest = HMAC-SHA256(storage_lookup_secret, verification_code_or_token)
reset_lookup_digest = HMAC-SHA256(storage_lookup_secret, reset_code_or_token)
```

Email addresses are normalized before hashing:

```text
email_digest = HMAC-SHA256(storage_lookup_secret, normalized_email)
```

The HMAC output is the lookup key. The raw token, code, state, session key, or email address must not appear in `pk` or `sk`.

## Security Invariants

- Raw authorization codes do not appear in keys.
- Raw refresh tokens do not appear in keys.
- Raw provider state values do not appear in keys.
- Raw OAuth session keys do not appear in keys.
- Raw verification or reset codes do not appear in keys.
- Raw email addresses do not appear in keys.
- Short-code and token lookup uses HMAC with a server-side secret.
