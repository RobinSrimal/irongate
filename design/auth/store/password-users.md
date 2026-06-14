# Password Users

Target code: `packages/functions/auth/src/store/password_users.rs`

## Owns

- Password user records.
- Email verification state updates.
- Password hash updates.

## Target Records

```text
password_user:<email_digest>
```

Value:

```json
{
  "email": "normalized@example.com",
  "password_hash": "...",
  "verified": false,
  "created_at": "...",
  "updated_at": "..."
}
```

## Security Invariants

- Key uses an email digest, not raw email.
- Password hash is the only stored password-derived value.
- Verification state changes only after consuming a valid verification secret.
- Password hash changes only after current-password validation or reset-secret consumption.
- Store methods should distinguish `create_unverified_user`, `mark_user_verified`, and `verify_login_password`.
- `verify_login_password` must return an error for unverified users when verification is required.
