# Passwords

Target code: `packages/functions/auth/src/core/passwords.rs`

## Owns

- Password policy.
- Password hash verification contract.
- Email verification requirement.
- Password reset domain rules.

## Target Behavior

Registration creates an unverified user with a password hash. Login can issue an OAuth authorization code only when the password is correct and the email verification requirement is satisfied.

Password reset changes the stored password hash only after a valid, single-use reset secret is consumed.

The core API must not expose a boolean that lets callers decide whether verification is required for a login. Verification policy belongs to the password module and configuration, not to individual route callers.

Expected domain flow:

```text
register_password_user(...) -> RegistrationPending
verify_password_email(...) -> VerifiedPasswordUser
login_password_user(...) -> VerifiedSubject
```

## Security Invariants

- Passwords are never stored or logged in plaintext.
- Password hashes use Argon2id with safe parameters.
- Registration does not authenticate an unverified email when verification is required.
- Registration never returns an authenticated subject or OAuth authorization code while the user is unverified.
- Login verification cannot be bypassed with a call-site flag.
- Password reset secrets are single-use and short-lived.
- Login and reset errors should limit email enumeration.

## Security Scan Coverage

This addresses the registration-verification bypass by making registration and login separate domain operations. The only operation that can produce an authenticated subject is the login operation, and it must enforce the configured verification requirement internally.
