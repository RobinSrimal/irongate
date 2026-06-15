# Infra Auth Functions

Target code: `infra/api.ts` Lambda route config plus `packages/functions/auth`.

## Owns

- Runtime selection for the Rust auth Lambdas.
- Memory, timeout, and architecture.
- Environment variables passed to auth.
- Links to DynamoDB and secrets.

## Target Behavior

The template deploys two Rust Lambdas behind one HTTP API:

```text
public auth Lambda
  /authorize
  /token
  /userinfo
  /oauth/revoke
  /password/*
  /google/*
  /apple/*

admin Lambda
  /_admin/*
```

The public auth Lambda owns browser/mobile/client-facing OAuth and identity-provider flows. The admin Lambda owns operator-only account lifecycle routes. Keeping admin in a separate Lambda gives a clearer control-plane boundary, avoids accidentally exposing admin handlers through `$default`, and lets admin runtime configuration avoid provider/email/signing secrets unless a route explicitly needs them.

API Gateway must enforce IAM on admin routes before invoking the admin Lambda. The admin Lambda should still reject requests if the expected API Gateway/IAM request context is absent.

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
- The public auth Lambda role should have only the DynamoDB, KMS, and secret permissions needed by public auth flows.
- The admin Lambda role should have only the DynamoDB and optional KMS permissions needed by lifecycle operations.
- Admin routes do not require custom admin secrets.

## Public Auth Runtime Dependencies

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

## Admin Runtime Dependencies

- DynamoDB table name.
- Audit logging mode and log retention settings.
- Deleted identity reuse settings when deletion is implemented.
- Optional KMS key references only where required by DynamoDB or future signing/secrets paths.

The admin Lambda should not receive Resend keys, provider client secrets, Apple private keys, or local JWT signing private keys unless a future route proves it needs them.
