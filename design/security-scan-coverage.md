# Security Scan Coverage

This document maps the current security scan findings to rewrite decisions.

Source report: `SECURITY_SCAN.md`

## C1: Admin Lifecycle Control Plane

Decision: account lifecycle operations use a separate IAM-protected admin Lambda.

Design coverage:

- `scope.md`
- OAuth clients are config-only.
- The public auth Lambda serves client-facing auth routes only.
- Account lifecycle admin routes are served by a separate admin Lambda and protected by API Gateway IAM authorization.

Implementation rule:

```text
Admin account lifecycle routes are not mounted in the public auth Lambda.
Admin account lifecycle routes require API Gateway IAM authorization and SigV4-signed requests before the admin Lambda is invoked.
```

The admin design uses AWS IAM authorization and sanitized lifecycle responses.

## C2: Password Registration Bypasses Required Email Verification

Decision: keep password auth, but make registration and login separate domain operations.

Design coverage:

- `functions/auth/core/passwords.md`
- `functions/auth/providers/password.md`
- `functions/auth/api/providers/password.md`
- `functions/auth/store/password-users.md`

Implementation rule:

```text
register email + password -> pending verification, no OAuth code
verify email -> mark user verified
login email + password -> only then issue OAuth authorization code
```

The target API must not expose a call-site boolean like `login(..., require_verified: false)`. Verification policy is enforced inside the password module.

## C3: Rate Limits Trust Forwarded IP Headers Under API Gateway

Decision: rate-limit source IP comes from API Gateway/Lambda request context, not forwarded headers.

Design coverage:

- `functions/auth/store/rate-limits.md`
- `infra/auth/api.md`

Implementation rule:

```text
x-forwarded-for is not a trusted source IP in API Gateway mode.
x-real-ip is not a trusted source IP in API Gateway mode.
```

Rate-limit keys should combine source IP with stronger identifiers where available, such as email digest or client ID.

## C4: One-Time Secret Expiry

Decision: authorization codes, provider states, verification links, and reset links use typed
one-time secret operations with explicit expiry.

Design coverage:

- `functions/auth/store/authorization-codes.md`
- `functions/auth/store/provider-states.md`
- `functions/auth/store/password-secrets.md`
- `functions/auth/store/dynamodb.md`

Implementation rule:

```text
authorization codes, provider states, verification secrets, and reset secrets store expires_at inside the record
authorization codes, provider states, verification secrets, and reset secrets store expiry as DynamoDB TTL
routes/providers never call generic set(..., None)
```

Every short-lived secret is created and consumed through purpose-specific typed store methods.

## Design Checklist

- IAM-protected admin routes are served by a separate admin Lambda and limited to account lifecycle operations.
- Verification policy is enforced inside password domain operations.
- Typed storage operations are exposed to route/provider code.
- Bearer secrets are stored by HMAC lookup digest.
- Runtime auth paths use exact-key reads, bounded queries, or transactions.
- Rate limits use API Gateway request-context source identity in API Gateway mode.
- Short-lived secret consume paths preserve and enforce expiry.
- Password registration tests prove no OAuth code is issued before verification.
- Rate-limit tests prove spoofed forwarded headers do not change the trusted source identity.
