# Auth Observability

Target code: `packages/functions/auth/src/observability`

## Owns

- Security event emission.
- Metrics emission.
- Safe structured logging helpers.

## Boundaries

- Token or code storage.
- Provider credentials.

Observability should help operate auth without exposing raw auth state.

## Defaults

V1 defaults to structured JSON logs through Lambda/CloudWatch.

```text
AUTH_AUDIT_LOG_MODE=cloudwatch
```

Developers may explicitly set:

```text
AUTH_AUDIT_LOG_MODE=none
```

to disable security audit event emission where they choose.
