# Scope

This document records the shape of the Irongate template.

Scope decisions live here because they cut across function code, infrastructure, and examples. The
design tree under `design/functions` and `design/infra` mirrors the code tree the template owns.

## What It Is

Irongate is a Rust and AWS auth template:

- Public auth Lambda for OAuth/OIDC, password auth, Google login, Apple login, refresh, revoke, and
  userinfo.
- IAM-protected admin Lambda for account lifecycle operations.
- DynamoDB-backed auth storage.
- SST-managed API Gateway, Lambda, DynamoDB, secrets, logs, and optional KMS resources.
- Optional examples for best-practice web BFF and native app integrations.

## Why

The template is intentionally small enough for developers to understand before they deploy it.
Security-sensitive behavior stays explicit:

- OAuth clients are config-only and reviewed through repo/deploy changes.
- Public auth routes never need a first-deployer bootstrap credential.
- Account lifecycle operations are isolated behind API Gateway IAM.
- DynamoDB stores typed auth records with HMAC lookup digests for bearer-style secrets.
- Email verification and password reset use Resend in every deployed stage.
- The auth core stays API-only so applications own their user-facing product experience.

## Core Auth Function

The public auth function owns:

- OAuth authorize, token, discovery, JWKS, revoke, and userinfo endpoints.
- OpenID Connect-compatible ID-token issuance.
- Config-only OAuth client loading and validation.
- Password registration, login, email verification, and password reset.
- Argon2id password hashing with length-based password policy.
- Google and Apple OIDC login.
- Persisted minimal account and identity records.
- Refresh-token rotation and user-facing refresh-token revocation.
- Rate limiting and audit logging.

The auth function returns protocol responses, JSON, redirects, and OAuth errors. Application login,
registration, reset, provider-selection, and error screens live in the consuming app or optional
examples.

## Admin Function

The admin function owns IAM-protected account lifecycle operations:

- Get sanitized account status.
- Disable a user.
- Enable a user.
- Delete a user.
- Revoke sessions for a user.

Operators call these routes with AWS Signature Version 4. API Gateway authorizes the request before
invoking the admin Lambda, and the admin Lambda returns sanitized account lifecycle responses rather
than raw DynamoDB records.

## Examples

Examples are optional reference implementations:

- `packages/examples/web`: Cloudflare Worker BFF using HttpOnly session cookies and Durable Object
  session storage.
- `packages/examples/app`: desktop-first Tauri app using PKCE, external browser login, loopback
  redirect, and OS keychain refresh-token storage.

Example infrastructure is opt-in through stage config. The default core deploy is only the auth/admin
Lambdas, API Gateway, DynamoDB, secrets, logs, and configured KMS resources.

## Tokens And APIs

Irongate issues self-contained JWT access tokens. Resource APIs validate issuer, audience, expiry,
signature, and scopes locally using discovery/JWKS metadata.

Disable and delete block login, refresh, userinfo, and new token issuance immediately. Already-issued
access tokens remain valid until expiry because they are self-contained JWTs.

ID tokens are for OIDC clients. Resource APIs should use access tokens for authorization and
row-level access control.
