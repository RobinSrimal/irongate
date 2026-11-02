# Password Verification And Reset Secrets

Implementation: `packages/functions/auth/src/store/password_secrets.rs`

## Owns

- Email verification link-token records.
- Password reset link-token records.
- HMAC lookup for verification and reset secrets.

## Behavior

Password registration creates an unverified password user and a verification secret. Forgot-password creates a reset secret.

Verification and reset use high-entropy link tokens.

The raw link token is sent by email once as part of a verification or reset URL. DynamoDB stores only a lookup digest:

```text
verification_lookup_digest = hex(HMAC-SHA256(storage_lookup_secret, "email_verification" || NUL || secret))
reset_lookup_digest = hex(HMAC-SHA256(storage_lookup_secret, "password_reset" || NUL || secret))
```

Verification record shape:

```json
{
  "email_digest": "...",
  "purpose": "verify_email",
  "created_at": "...",
  "expires_at": "..."
}
```

Reset record shape:

```json
{
  "email_digest": "...",
  "subject": "user_...",
  "purpose": "reset_password",
  "created_at": "...",
  "expires_at": "..."
}
```

Email verification expiry is derived from `AUTH_EMAIL_VERIFICATION_TTL_SECONDS`. Password reset expiry is derived from `AUTH_PASSWORD_RESET_TTL_SECONDS`. Both are written inside the record and as the DynamoDB `expiry` attribute.

## Store Operations

```text
create_email_verification
consume_email_verification
create_password_reset
consume_password_reset
delete_password_secrets_for_subject
```

## Security Invariants

- Raw verification and reset secrets never appear in `pk`, `sk`, logs, or errors.
- Verification and reset link tokens are high entropy.
- Verification and reset link tokens are short-lived.
- Verification and reset secrets are single-use.
- Expired records are rejected even if DynamoDB TTL has not deleted them.
- Consuming a verification secret is the only path that marks a password user verified.
- Consuming a reset secret is the only reset-token path that updates a password hash.
- Account disable/delete paths can remove reset secrets for the subject.

## Consumption Boundary

Link tokens are created and consumed through purpose-specific store operations, not generic writes.
Expiry is stored in the record and enforced during consume.
Proof consumption and the resulting password/account update are separate operations, not one
recovery transaction.
