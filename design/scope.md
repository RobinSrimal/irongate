# Scope

This document records what is inside and outside the first production-ready template.

Scope decisions live here because they do not map to target code folders. The design tree under `design/auth` and `design/infra` should mirror code we intend to create.

## In Scope

- OAuth authorize, token, discovery, JWKS, and userinfo.
- Password registration, login, email verification, and password reset.
- Google OIDC login.
- Apple OIDC login.
- Refresh token rotation.
- Concrete DynamoDB auth store.
- Resend email delivery.
- Rate limiting.
- SST deployment to API Gateway, Lambda, DynamoDB, secrets, and optional KMS.

## Out Of Initial Core

- Runtime admin API.
- Public `/admin/bootstrap`.
- Payments.
- Email OTP or magic-link login.
- Generic arbitrary OAuth2 identity providers.
- Local/console email delivery.
- Generic runtime storage providers beyond DynamoDB.

## Runtime Admin API

Status: out of initial core.

A runtime admin API creates a control plane inside the auth service. The current public bootstrap flow is high risk and should not be part of the minimal template.

Preferred first version:

- OAuth clients defined through code/config/deployment.
- No public bootstrap.
- No standing admin API key.

If runtime admin returns later, it requires:

- Non-public bootstrap or deploy-time key creation.
- Least-privilege admin permissions.
- Conditional writes for first-key creation.
- Strong audit logging.
- Tests proving unauthenticated callers cannot create admin credentials.

## Email OTP Or Magic Link

Status: out of initial core.

The first-party login method is password auth. Email remains part of the system for verification and password reset, but not as the primary login factor.

If OTP or magic-link login returns later, it requires:

- HMAC lookup digests for codes or links.
- Short TTL and single-use consumption.
- Strong rate limits by email and source.
- No raw codes or links in DynamoDB keys.
- Explicit product decision about whether email alone is sufficient for login.

## Generic OAuth2 Identity Providers

Status: out of initial core.

The target core supports first-class Google and Apple OIDC providers. Generic OAuth2 is not treated as identity because OAuth2 alone does not provide the same identity-token validation model.

If generic providers return later, they should be OIDC-only for identity use, with issuer, audience, nonce, and JWKS validation defined per provider.
