# Password Identity Provider

Target code: `packages/functions/auth/src/providers/password.rs`

## Owns

- Password registration.
- Password hash verification.
- Email verification state.
- Password reset state.
- Mapping verified password users to internal identity.

## Target Behavior

Password login verifies that the caller knows the password for a registered and verified email address.

Registration creates an unverified user. The user becomes eligible for token issuance only after email verification.

Registration is not login. A successful registration returns pending-verification state, not an authenticated identity.

After verification, the password identity maps to a generated persisted subject. Login must fail if that subject's account status is disabled or deleted.

## Security Invariants

- Passwords are hashed with Argon2id.
- Passwords must be 12 to 128 characters.
- Password policy does not require character composition rules.
- Breached-password checking is out of v1.
- Verification and reset link tokens are high-entropy and single-use.
- Verification and reset link tokens expire quickly.
- Login, registration, verification, and reset attempts are rate-limited.
- Normalized email is used consistently.
- Email verification is required before issuing tokens when enabled.
- Email delivery failures do not mark users verified.
- Disabled or deleted accounts cannot log in.
