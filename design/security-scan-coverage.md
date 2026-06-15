# Security Scan Coverage

This document maps the current security scan findings to rewrite decisions.

Source report: `SECURITY_SCAN.md`

## C1: Public Bootstrap Endpoint Mints A Full Admin Key

Decision: remove runtime admin bootstrap and custom admin keys from the first auth core.

Design coverage:

- `scope.md`
- OAuth clients are config-only for the first version.
- The public auth Lambda has no first-deployer-wins admin credential route.
- Account lifecycle admin routes are served by a separate admin Lambda and protected by API Gateway IAM authorization.

Implementation rule:

```text
No POST /admin/bootstrap route in the target core.
No standing admin API key required for normal operation.
No runtime client creation or client-secret rotation endpoint in the target core.
Admin account lifecycle routes are not mounted in the public auth Lambda.
Admin account lifecycle routes require API Gateway IAM authorization and SigV4-signed requests before the admin Lambda is invoked.
```

If broader runtime admin returns later, it needs a separate design using deployer-controlled auth, least-privilege permissions, and audit logging. It must not reintroduce a public first-deployer-wins credential flow.

## C2: Password Registration Bypasses Required Email Verification

Decision: keep password auth, but make registration and login separate domain operations.

Design coverage:

- `auth/core/passwords.md`
- `auth/providers/password.md`
- `auth/api/providers/password.md`
- `auth/store/password-users.md`

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

- `auth/store/rate-limits.md`
- `infra/api.md`

Implementation rule:

```text
x-forwarded-for is not a trusted source IP in API Gateway mode.
x-real-ip is not a trusted source IP in API Gateway mode.
```

Rate-limit keys should combine source IP with stronger identifiers where available, such as email digest or client ID.

## C4: OTP Failed Attempts Remove Expiration From The Code Record

Decision: passwordless OTP and short verification/reset codes are not in the target core. Verification and reset use high-entropy link tokens, so there are no failed-attempt counter updates on that path.

Design coverage:

- `auth/store/authorization-codes.md`
- `auth/store/provider-states.md`
- `auth/store/password-secrets.md`
- `auth/store/dynamodb.md`

Implementation rule:

```text
authorization codes, provider states, verification secrets, and reset secrets store expires_at inside the record
authorization codes, provider states, verification secrets, and reset secrets store expiry as DynamoDB TTL
routes/providers never call generic set(..., None)
```

Every short-lived secret is created and consumed through purpose-specific typed store methods.

## Rewrite Checklist

- No runtime admin bootstrap route.
- No custom admin API key.
- IAM-protected admin routes are served by a separate admin Lambda and limited to account lifecycle operations.
- No route-controlled verification bypass.
- No generic storage operations exposed to route/provider code.
- No raw bearer secrets in DynamoDB keys.
- No unbounded scans in runtime auth paths.
- No forwarded-header source IP trust in API Gateway mode.
- Short-lived secret consume paths preserve and enforce expiry.
- Password registration tests prove no OAuth code is issued before verification.
- Rate-limit tests prove spoofed forwarded headers do not change the trusted source identity.
