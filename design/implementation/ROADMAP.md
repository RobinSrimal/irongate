# Implementation Roadmap

This roadmap turns the design tree into implementation slices. Each slice should leave the repo in a coherent state with focused tests, not just move code around.

## Principles

- Keep the auth core API-only.
- Keep app and UI decisions deferred.
- Prefer vertical, testable slices over broad rewrites.
- Keep one Rust auth Lambda, one API Gateway HTTP API, and DynamoDB as the default runtime shape.
- Replace generic behavior with typed modules before adding new auth flows on top.
- Do not introduce runtime client management, public bootstrap, passwordless OTP, token introspection, opaque access tokens, or generic OAuth identity providers in v1.

## Slice Sequence

### 01_foundation_config_store_and_discovery

Build the secure foundation that all later flows depend on:

- Runtime config validation.
- Config-only OAuth client registry.
- Typed DynamoDB store facade.
- HMAC lookup helpers.
- Generated subjects and account/identity records.
- Signing abstraction with local ES256 support.
- OIDC/OAuth discovery and JWKS endpoints.

This slice creates deployable, testable behavior without implementing login yet.

### 02_password_registration_verification_and_login

Implement the first-party password flow:

- Register with email and password.
- Argon2id password hashing.
- Resend verification email.
- Verification link token storage and consumption.
- Login only after verification.
- Authorization-code issuance after successful login.

### 03_token_exchange_refresh_userinfo_and_logout

Complete the first-party OAuth/OIDC token loop:

- Authorization-code exchange.
- Access-token and ID-token issuance.
- Refresh-token rotation and reuse detection.
- `/userinfo`.
- `/oauth/revoke` for user-facing logout.

### 04_google_and_apple_oidc_login

Add external identity providers:

- Google OIDC start/callback flow.
- Apple OIDC start/callback flow.
- Provider state and nonce handling.
- Issuer + subject identity mapping.
- No auto-linking by email.

### 05_iam_admin_account_lifecycle

Add operator account lifecycle routes:

- IAM-protected `/_admin/*` routes.
- Disable user.
- Delete user with fixed anonymized tombstone behavior.
- Revoke all sessions for a subject.
- Deleted identity reuse policy.

### 06_aws_hardening_and_runtime_validation

Tighten deployment behavior around AWS:

- API Gateway request-context source IP for rate limits.
- Least-privilege IAM.
- CloudWatch audit logging defaults and opt-out.
- Optional customer managed DynamoDB KMS key.
- KMS ES256 signing mode.
- AWS dev deployment smoke tests.

### 07_legacy_removal_and_security_regression

Finish the rewrite:

- Remove old UI rendering from the auth Lambda.
- Remove generic runtime storage paths.
- Remove memory storage as a runtime option.
- Remove unsafe admin bootstrap/client management paths.
- Run targeted security regression tests.
- Confirm DynamoDB key shapes contain no raw bearer values.

## Definition Of Done For Each Slice

Every slice should include:

- Code changes scoped to the slice.
- Unit tests for pure Rust modules.
- Store tests for DynamoDB key and expiry behavior where relevant.
- HTTP handler tests for public API behavior where relevant.
- Config validation tests for new settings.
- Updated docs when the implemented behavior changes the design.
- A short manual validation note if AWS behavior is involved.

## Deferred Decisions

- App/UI/reference website shape.
- Payments.
- Generic OIDC provider registry beyond Google and Apple.
- Machine-to-machine `client_credentials`.
- Token introspection or opaque access tokens.
- Hosted UI.

