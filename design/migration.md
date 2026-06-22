# Migration And Refactor Plan

This document describes how the repository moves toward the current target shape.

## Target Shape

The template converges on:

```text
HTTP API
  -> public Rust auth Lambda
  -> IAM-protected Rust admin Lambda
  -> DynamoDB AuthTable
  -> SST secrets, logs, and optional KMS
```

Optional examples live behind explicit stage enablement:

```text
packages/examples/web
packages/examples/app
infra/examples
```

## Refactor Direction

| Area | Target shape |
| --- | --- |
| Storage | Concrete typed DynamoDB auth store |
| Token/code lookup | HMAC lookup digests |
| OAuth clients | Config-only client registry |
| Password registration | Pending verification response |
| Source IP | API Gateway request context source IP |
| Email delivery | Resend in dev and production |
| Auth UI | API-only endpoints and configurable email templates |
| Admin lifecycle | Separate IAM-protected admin Lambda |
| JWT signing | KMS signing or local ES256 private key outside AuthTable |

## Implementation Order

1. Introduce typed store modules and records.
2. Move auth code, provider state, refresh token, password user, identity, verification/reset storage
   into typed store operations.
3. Replace generic storage calls in routes/providers.
4. Move OAuth client lookup to a validated read-only config registry.
5. Add `auth.clients.toml` for non-secret client definitions and SST secret refs for confidential
   clients.
6. Add generated persisted subject IDs and account lifecycle records.
7. Add separate admin Lambda with IAM-protected `/_admin/*` account lifecycle routes.
8. Add deleted identity reuse and retention configuration.
9. Add user-facing `/oauth/revoke` refresh-token revocation.
10. Add Resend-only email delivery.
11. Add configurable verification/reset email templates.
12. Add config validation for required secrets, template paths, client definitions, and deleted
    identity reuse policy.
13. Add security regression tests.
14. Deploy to AWS dev and validate API Gateway IAM, source IP, and DynamoDB key shape.
15. Restructure infra into `infra/auth`, `infra/shared`, and disabled-by-default `infra/examples`.
16. Keep optional example applications focused on auth integration.

## Compatibility

The repository is pre-production, so the rewrite can break WIP data. Production compatibility should
be handled by an explicit migration design before any stable release promises are made.
