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
  -> create verification code/link
  -> send verification email
  -> return verification_required
  -> no OAuth authorization code

verify email
  -> consume verification code/link
  -> mark user verified

login email + password
  -> verify password hash
  -> require verified email
  -> issue OAuth authorization code

forgot password
  -> create reset code/link
  -> send reset email

reset password
  -> consume reset code/link
  -> store new password hash
```

## Security Invariants

- Registration must not issue tokens before email verification when verification is required.
- Registration responses must not include an OAuth code, access token, refresh token, or authenticated subject.
- Passwords are stored only as Argon2id hashes.
- Verification and reset secrets are short-lived and single-use.
- Verification and reset lookups use HMAC digests, not raw codes in keys.
- Login, registration, verification, and reset attempts are rate-limited.
- Error responses should limit email enumeration.
