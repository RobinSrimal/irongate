# Migration And Refactor Plan

This document describes how the current repository should move toward the target design.

## Remove From Target Core

- Public `/admin/bootstrap`.
- Public/custom-key runtime admin API.
- Runtime OAuth client management.
- OAuth `client_credentials` grant.
- OAuth token introspection endpoint.
- Opaque access tokens.
- Breached-password API integration.
- Passwordless OTP/code provider.
- Generic arbitrary OAuth2 provider as an identity provider.
- Generic `StorageAdapter` exposed to route/provider code.
- `MemoryStorage` as a runtime storage option.
- Built-in HTML auth UI modules.
- Payments.

## Keep And Rewrite

- OAuth authorize/token/userinfo/discovery/JWKS.
- OpenID Connect-compatible ID-token issuance.
- Self-contained JWT access tokens.
- Config-only OAuth clients.
- Password registration/login/verification/reset.
- Argon2id password hashing with length-based password policy.
- Google OIDC.
- Apple OIDC.
- Persisted minimal identity records.
- Refresh token rotation.
- User-facing refresh token revocation for logout.
- Rate limiting.
- Configurable CloudWatch audit logging.
- Configurable verification/reset email templates.
- IAM-protected account lifecycle admin routes.
- DynamoDB table.
- SST API Gateway/Lambda/DynamoDB deployment.

## Replace

| Current | Target |
| --- | --- |
| Generic storage adapter | Concrete typed DynamoDB auth store |
| Raw token/code keys | HMAC lookup digests |
| Public admin bootstrap | Config-only clients and IAM-protected account lifecycle routes |
| Runtime client CRUD | Config-only client registry |
| Password registration auto-login | Pending verification response |
| Forwarded-header source IP | API Gateway request context source IP |
| Local/console email behavior | Resend in dev and prod |
| Built-in HTML auth pages | API-only endpoints and configurable email templates |
| JWT private key in AuthTable | KMS signing or encrypted private key outside ordinary table reads |

## Suggested Order

1. Introduce typed store modules and records.
2. Move auth code, provider state, refresh token, password user, identity, verification/reset storage into typed store operations.
3. Replace generic storage calls in routes/providers.
4. Remove passwordless OTP/code provider from default core.
5. Remove admin/bootstrap routes from target router.
6. Move OAuth client lookup to a validated read-only config registry.
7. Add `auth.clients.toml` for non-secret client definitions and SST secret refs for confidential clients.
8. Add generated persisted subject IDs and account lifecycle records.
9. Add IAM-protected `/_admin/*` account lifecycle routes.
10. Add deleted identity reuse and retention configuration.
11. Add user-facing `/oauth/revoke` refresh token revocation.
12. Add Resend-only email delivery.
13. Remove built-in auth page rendering from the target router.
14. Add configurable verification/reset email templates.
15. Add config validation for required secrets, template paths, client definitions, and deleted identity reuse policy.
16. Add security regression tests.
17. Deploy to AWS dev and validate API Gateway IAM, source IP, and DynamoDB key shape.

## Compatibility

The rewrite is allowed to break compatibility with existing WIP data because this repo is still pre-production. If compatibility becomes necessary later, add an explicit migration design before implementation.
