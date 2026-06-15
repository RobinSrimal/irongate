# Scope

This document records what is inside and outside the first production-ready template.

Scope decisions live here because they do not map to target code folders. The design tree under `design/auth` and `design/infra` should mirror code we intend to create.

## In Scope

- OAuth authorize, token, discovery, JWKS, and userinfo.
- OpenID Connect compatibility for standard OIDC clients.
- Config-only OAuth clients.
- Password registration, login, email verification, and password reset.
- Argon2id password hashing with length-based password policy.
- Google OIDC login.
- Apple OIDC login.
- Persisted minimal identity records.
- Refresh token rotation.
- User-facing refresh token revocation for logout.
- Concrete DynamoDB auth store.
- Resend email delivery.
- Configurable verification and reset email templates.
- IAM-protected admin account lifecycle operations.
- Configurable deleted identity reuse policy.
- Rate limiting.
- Configurable CloudWatch audit logging.
- SST deployment to API Gateway, Lambda, DynamoDB, secrets, and optional KMS.

## Out Of Initial Core

- Public or custom-key runtime admin API.
- Runtime OAuth client management.
- OAuth `client_credentials` grant.
- OAuth token introspection endpoint.
- Opaque access tokens.
- Breached-password API integration.
- Public `/admin/bootstrap`.
- Payments.
- Email OTP or magic-link login.
- Generic arbitrary OAuth2 identity providers.
- Local/console email delivery.
- Generic runtime storage providers beyond DynamoDB.
- Built-in login, registration, reset, or provider-selection UI.

## Frontend-Agnostic Foundation

Status: intentional product boundary.

The auth foundation should not force a frontend framework, hosted UI implementation, or frontend deployment model. It provides the Rust + AWS auth backend and OIDC protocol surface.

App and UI decisions are intentionally deferred.

## OAuth Clients

Status: in initial core as config-only.

OAuth clients are the applications allowed to use the auth server, such as a web app, mobile app, or backend client. In the first template, they are declared in repo/deployment configuration and changed by redeploying.

The auth API does not create, update, rotate, disable, or delete clients at runtime. That avoids needing bootstrap admin credentials or a custom client-management control plane.

## OpenID Connect Compatibility

Status: in initial core.

The goal is a Rust and AWS OpenAuth replacement, so the auth service should expose standard OIDC protocol endpoints. V1 should include OpenID Connect discovery metadata, JWKS, authorization-code flow, `openid` scope handling, ID-token issuance, and userinfo.

V1 remains API-only. It does not include hosted login, consent, account-selection, or provider-selection pages. Applications using the template own that UI and must drive the auth flow through the documented API/provider endpoints.

ID tokens are for OIDC clients. APIs should use access tokens for authorization and row-level access control.

V1 access tokens are self-contained JWTs. Resource APIs validate them locally using issuer, audience, expiry, signature, and scopes. Disable/delete blocks login, refresh, userinfo, and new token issuance immediately, but already-issued access tokens remain valid until expiry. Token introspection is out of v1.

## Built-In Auth UI

Status: out of initial core.

The first template is API-only. Applications built on the template own their user-facing login, registration, password reset, provider selection, and error screens.

Email templates remain in scope because verification and reset emails are part of the auth workflow. Those templates are deployment-configurable message bodies, not an embedded web UI.

If hosted UI becomes a product goal later, it needs its own design. It should not be introduced implicitly through the auth foundation.

## Admin API

Status: narrowly in scope for account lifecycle only.

Public or custom-key runtime admin APIs are out of the initial core. The current public bootstrap flow is high risk and should not be part of the minimal template.

Preferred first version:

- Config-only OAuth clients.
- No public bootstrap.
- No standing admin API key.
- Separate admin Lambda for IAM-protected `/_admin/*` user/account lifecycle routes.

The only v1 admin operations are:

- Get sanitized account status.
- Disable a user.
- Delete a user.
- Revoke sessions for a user.

These routes are protected by API Gateway IAM authorization and AWS Signature Version 4. IAM policies grant `execute-api:Invoke` to operator roles for specific admin route ARNs. API Gateway rejects unsigned or unauthorized requests before invoking the admin Lambda. The public auth Lambda must not mount these lifecycle handlers behind `$default`.

Runtime client management remains out of v1. If broader runtime admin returns later, it requires:

- Non-public bootstrap or deploy-time key creation.
- Least-privilege admin permissions.
- Conditional writes for first-key creation.
- Strong audit logging.
- Tests proving unauthenticated callers cannot create admin credentials.

## Email OTP Or Magic Link

Status: out of initial core.

The first-party login method is password auth. Email remains part of the system for verification and password reset, but not as the primary login factor.

If OTP or magic-link login returns later, it requires:

- HMAC lookup digests for tokens.
- Short TTL and single-use consumption.
- Strong rate limits by email and source.
- No raw tokens in DynamoDB keys.
- Explicit product decision about whether email alone is sufficient for login.

## Generic OAuth2 Identity Providers

Status: out of initial core.

The target core supports first-class Google and Apple OIDC providers. Generic OAuth2 is not treated as identity because OAuth2 alone does not provide the same identity-token validation model.

If generic providers return later, they should be OIDC-only for identity use, with issuer, audience, nonce, and JWKS validation defined per provider.
