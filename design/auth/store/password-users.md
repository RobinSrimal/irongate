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
  "subject": "optional user:... after verification",
  "password_hash": "...",
  "password_hash_updated_at": "...",
  "verified": false,
  "created_at": "...",
  "updated_at": "..."
}
```

## Security Invariants

- Key uses an email digest, not raw email.
- Password hash is the only stored password-derived secret.
- Password hash is stored as a PHC string.
- Verification state changes only after consuming a valid verification secret.
- Email verification creates or attaches the generated account subject for the password identity.
- Password hash changes only after current-password validation or reset-secret consumption.
- Deletion removes password hash material and contact metadata according to the fixed anonymized tombstone policy.
- Successful login may update the stored password hash when current Argon2id parameters require rehash.
- Store methods should distinguish `create_unverified_user`, `mark_user_verified`, and `verify_login_password`.
- `verify_login_password` must return an error for unverified users when verification is required.
