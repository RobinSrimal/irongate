# Infra Auth Function

Target code: `infra/api.ts` Lambda route config plus `packages/functions/auth`.

## Owns

- Runtime selection for the Rust Lambda.
- Memory, timeout, and architecture.
- Environment variables passed to auth.
- Links to DynamoDB and secrets.

## Target Behavior

The template deploys one fat auth Lambda initially. This is acceptable because the auth surface is small and the Rust handler can share warmed clients across invocations.

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

## Runtime Dependencies

- DynamoDB table name.
- Issuer URL.
- Provider configuration.
- HMAC lookup secret.
- `RESEND_API_KEY`.
- `AUTH_EMAIL_FROM`.
- Optional `AUTH_EMAIL_REPLY_TO`.
- Optional KMS key references.
