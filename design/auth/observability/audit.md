# Audit Events

Target code: `packages/functions/auth/src/observability/audit.rs`

## Owns

- Security-relevant event definitions.
- Sanitized audit event emission.

## Target Events

- Login success.
- Login failure.
- Email verification requested.
- Password reset requested.
- Provider callback failure.
- Refresh token reuse.
- Token revocation.
- Rate-limit exceeded.

## Security Invariants

- No raw tokens, codes, passwords, or private keys.
- Token references use hashes.
- Audit data should be separate from raw auth state.
