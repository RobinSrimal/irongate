# Password Provider API

Target code: `packages/functions/auth/src/api/providers/password.rs`

## Owns

- Password registration endpoint.
- Password login endpoint.
- Email verification endpoint.
- Password reset request and reset completion endpoints.
- Calling the email delivery module for verification and reset messages.

## Target Flow

```text
register email + password
  -> create password user with verified=false
  -> create verification link token
  -> send verification email
  -> return verification_required
  -> no OAuth authorization code

verify email
  -> consume verification link token
  -> mark user verified

login email + password
  -> verify password hash
  -> require verified email
  -> require active account
  -> issue OAuth authorization code

forgot password
  -> create reset link token
  -> send reset email

reset password
  -> consume reset link token
  -> store new password hash
```

## Security Invariants

- Registration must not issue tokens before email verification when verification is required.
- Registration responses must not include an OAuth code, access token, refresh token, or authenticated subject.
- Password policy is enforced before creating or updating a password hash.
- Passwords are stored only as Argon2id hashes.
- Verification and reset link tokens are high-entropy, short-lived, and single-use.
- Verification and reset lookups use HMAC digests, not raw link tokens in keys.
- Login, registration, verification, and reset attempts are rate-limited.
- Disabled or deleted accounts cannot receive an OAuth authorization code.
- Error responses should limit email enumeration.
