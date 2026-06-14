# Audit Events

Target code: `packages/functions/auth/src/observability/audit.rs`

## Owns

- Security-relevant event definitions.
- Sanitized audit event emission.
- Audit logging mode.

## Decision

V1 audit events are emitted as structured JSON logs to CloudWatch by default in every stage.

Audit persistence is config-based:

```text
AUTH_AUDIT_LOG_MODE=cloudwatch
AUTH_AUDIT_LOG_MODE=none
```

Default:

```text
AUTH_AUDIT_LOG_MODE=cloudwatch
```

`none` is an explicit opt-out. It disables security audit event emission, but it should not disable ordinary Lambda error logging.

Log retention is controlled by infrastructure configuration, not by the Rust audit emitter.

## Target Events

- Registration started.
- Email verification completed.
- Password login succeeded.
- Password login failed.
- Password reset requested.
- Password reset completed.
- Google login succeeded.
- Apple login succeeded.
- Provider login failed.
- Authorization code exchanged.
- Refresh token rotated.
- Refresh token reuse detected.
- Refresh family revoked.
- User logout refresh token revoked.
- Rate-limit exceeded.

## Event Shape

Audit events should be compact JSON records:

```json
{
  "kind": "audit",
  "event": "password_login_failed",
  "timestamp": "...",
  "request_id": "...",
  "client_id": "optional",
  "subject": "optional",
  "provider": "optional",
  "source": "coarse source identity",
  "result": "success|failure",
  "reason": "safe coarse reason"
}
```

The `source` value should come from the trusted API Gateway request context path used by rate limiting, not spoofable forwarded headers.

## Security Invariants

- No raw tokens, codes, passwords, or private keys.
- No verification or reset links.
- No provider access tokens or ID tokens.
- No client secrets.
- Token references use hashes.
- Audit data should be separate from raw auth state.
- `AUTH_AUDIT_LOG_MODE=none` must be visible in startup logs without printing secrets.
