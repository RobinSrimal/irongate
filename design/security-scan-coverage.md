# Security Scan Coverage

This document maps the current security scan findings to rewrite decisions.

Source report: `SECURITY_SCAN.md`

## C1: Public Bootstrap Endpoint Mints A Full Admin Key

Decision: remove runtime admin bootstrap from the first auth core.

Design coverage:

- `scope.md`
- OAuth clients are deployment-defined for the first version.
- The public auth Lambda has no first-deployer-wins admin credential route.

Implementation rule:

```text
No POST /admin/bootstrap route in the target core.
No standing admin API key required for normal operation.
```

If runtime admin returns later, it needs a separate design using deployer-controlled auth, conditional first-key creation, least-privilege permissions, and audit logging.

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

Decision: passwordless OTP is not in the target core, but the expiry-loss class applies to verification/reset secrets and must be prevented centrally.

Design coverage:

- `auth/store/authorization-codes.md`
- `auth/store/provider-states.md`
- `auth/store/password-secrets.md`
- `auth/store/dynamodb.md`

Implementation rule:

```text
authorization codes, provider states, verification secrets, and reset secrets store expires_at inside the record
authorization codes, provider states, verification secrets, and reset secrets store expiry as DynamoDB TTL
failed-attempt updates preserve both values
routes/providers never call generic set(..., None)
```

Every short-lived secret is created, consumed, and updated through purpose-specific typed store methods.

## Rewrite Checklist

- No runtime admin bootstrap route.
- No route-controlled verification bypass.
- No generic storage operations exposed to route/provider code.
- No raw bearer secrets in DynamoDB keys.
- No unbounded scans in runtime auth paths.
- No forwarded-header source IP trust in API Gateway mode.
- Short-lived secret attempt updates preserve expiry.
- Password registration tests prove no OAuth code is issued before verification.
- Rate-limit tests prove spoofed forwarded headers do not change the trusted source identity.
