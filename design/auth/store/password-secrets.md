# Password Verification And Reset Secrets

Target code: `packages/functions/auth/src/store/password_secrets.rs`

## Owns

- Email verification code or link records.
- Password reset code or link records.
- HMAC lookup for verification and reset secrets.
- Attempt counters for verification and reset.

## Target Behavior

Password registration creates an unverified password user and a verification secret. Forgot-password creates a reset secret.

The raw code or link token is sent by email once. DynamoDB stores only a lookup digest:

```text
verification_lookup_digest = HMAC-SHA256(storage_lookup_secret, "password_verify:" || secret)
reset_lookup_digest = HMAC-SHA256(storage_lookup_secret, "password_reset:" || secret)
```

Verification record shape:

```json
{
  "email_digest": "...",
  "purpose": "verify_email",
  "attempts": 0,
  "max_attempts": 5,
  "created_at": "...",
  "expires_at": "..."
}
```

Reset record shape:

```json
{
  "email_digest": "...",
  "purpose": "reset_password",
  "attempts": 0,
  "max_attempts": 5,
  "created_at": "...",
  "expires_at": "..."
}
```

## Store Operations

```text
create_email_verification
consume_email_verification
record_failed_email_verification_attempt
create_password_reset
consume_password_reset
record_failed_password_reset_attempt
```

## Security Invariants

- Raw verification and reset secrets never appear in `pk`, `sk`, logs, or errors.
- Verification and reset secrets are short-lived.
- Verification and reset secrets are single-use.
- Attempt updates preserve both the record `expires_at` field and the DynamoDB `expiry` attribute.
- Expired records are rejected even if DynamoDB TTL has not deleted them.
- Consuming a verification secret is the only path that marks a password user verified.
- Consuming a reset secret is the only reset-token path that updates a password hash.

## Security Scan Coverage

This prevents the expiry-loss class found in the current OTP provider. Attempt updates are purpose-specific store operations, not generic writes.
