# Migration And Refactor Plan

This document describes how the current repository should move toward the target design.

## Remove From Target Core

- Public `/admin/bootstrap`.
- Runtime admin API.
- Passwordless OTP/code provider.
- Generic arbitrary OAuth2 provider as an identity provider.
- Generic `StorageAdapter` exposed to route/provider code.
- `MemoryStorage` as a runtime storage option.
- Payments.

## Keep And Rewrite

- OAuth authorize/token/userinfo/discovery/JWKS.
- Password registration/login/verification/reset.
- Google OIDC.
- Apple OIDC.
- Refresh token rotation.
- Rate limiting.
- DynamoDB table.
- SST API Gateway/Lambda/DynamoDB deployment.

## Replace

| Current | Target |
| --- | --- |
| Generic storage adapter | Concrete typed DynamoDB auth store |
| Raw token/code keys | HMAC lookup digests |
| Public admin bootstrap | Deployment-defined clients |
| Password registration auto-login | Pending verification response |
| Forwarded-header source IP | API Gateway request context source IP |
| Local/console email behavior | Resend in dev and prod |
| JWT private key in AuthTable | KMS signing or encrypted private key outside ordinary table reads |

## Suggested Order

1. Introduce typed store modules and records.
2. Move auth code, provider state, refresh token, password user, verification/reset storage into typed store operations.
3. Replace generic storage calls in routes/providers.
4. Remove passwordless OTP/code provider from default core.
5. Remove admin/bootstrap routes from target router.
6. Add Resend-only email delivery.
7. Add config validation for required secrets.
8. Add security regression tests.
9. Deploy to AWS dev and validate API Gateway source IP and DynamoDB key shape.

## Compatibility

The rewrite is allowed to break compatibility with existing WIP data because this repo is still pre-production. If compatibility becomes necessary later, add an explicit migration design before implementation.
