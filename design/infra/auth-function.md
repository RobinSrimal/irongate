# Infra Auth Function

Target code: `infra/api.ts` Lambda route config plus `packages/functions/auth`.

## Owns

- Runtime selection for the Rust Lambda.
- Memory, timeout, and architecture.
- Environment variables passed to auth.
- Links to DynamoDB and secrets.

## Target Behavior

The template deploys one fat auth Lambda initially. This is acceptable because the auth surface is small and the Rust handler can share warmed clients across invocations.

The same Rust Lambda may handle public auth routes and IAM-protected `/_admin/*` account lifecycle routes. API Gateway must enforce IAM on the admin routes before invocation, and the Lambda should reject admin paths if the expected IAM request context is absent.

Default shape:

```text
runtime: rust
architecture: arm64
memory: 256 MB initially
timeout: 30 seconds initially
```

Memory can be increased after measuring cold start and token-flow latency.

The auth runtime should reuse AWS SDK and HTTP clients across warm Lambda invocations. See `performance.md`.

## Security Invariants

- `DEV_MODE` must never be true in production.
- Provider secrets must not be embedded in source files.
- Logs must be JSON and must not print bearer tokens, auth codes, reset codes, or private keys.
- The Lambda role should have only the DynamoDB, KMS, and secret permissions needed by auth.
- Admin routes do not require custom admin secrets.

## Runtime Dependencies

- DynamoDB table name.
- Issuer URL.
- Provider configuration.
- Client config file path.
- HMAC lookup secret.
- Token and short-lived record TTL settings.
- Audit logging mode and log retention settings.
- `RESEND_API_KEY`.
- `AUTH_EMAIL_FROM`.
- Optional `AUTH_EMAIL_REPLY_TO`.
- Optional email branding, subject, and template path settings.
- Optional KMS key references.
