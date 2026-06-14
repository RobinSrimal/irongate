# Auth Observability

Target code: `packages/functions/auth/src/observability`

## Owns

- Security event emission.
- Metrics emission.
- Safe structured logging helpers.

## Must Not Own

- Token or code storage.
- Provider credentials.

Observability should help operate auth without exposing raw auth state.
