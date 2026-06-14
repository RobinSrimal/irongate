# Email Templates

Target code: `packages/functions/auth/src/email/templates.rs`

## Owns

- Email verification body.
- Password reset body.
- Subject lines and safe interpolation.

## Target Behavior

Templates should be simple and deterministic. They should include only the minimum information needed to verify an email address or reset a password.

## Security Invariants

- Escape all user-controlled display values.
- Do not include secrets other than the intended verification or reset code/link.
- Avoid logging rendered templates.
- Links should be short-lived and bound to the stored verification or reset record.
