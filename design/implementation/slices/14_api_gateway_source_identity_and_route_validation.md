# 14_api_gateway_source_identity_and_route_validation

## Goal

Remove spoofable forwarded-header trust from rate-limit source identity in API Gateway mode, and add validation around the deployed API route shape.

At the end of this slice, the auth Lambda should derive source IP for rate limits and audit source fields from the API Gateway/Lambda request context, not from `x-forwarded-for` or `x-real-ip`. The SST API definition should also have focused tests or static validation proving public auth routes stay public, admin lifecycle routes stay IAM-protected, and admin routes invoke the separate admin Lambda.

This is the first AWS hardening slice. It intentionally does not include KMS, customer-managed DynamoDB keys, KMS JWT signing, broad IAM policy reduction, or live production deployment.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/store/rate-limits.md`
- `design/auth/observability/audit.md`
- `design/auth/testing.md`
- `design/infra/api.md`
- `design/infra/auth-function.md`
- `design/infra/iam.md`
- `design/infra/stages.md`
- `design/scope.md`

The important design constraint is that API Gateway mode must use API Gateway request context as the trusted source identity. Forwarded headers are request headers and are not trusted rate-limit inputs in this deployed shape.

## Why This Slice Next

The auth and lifecycle foundations now exist: password, Google, Apple, token exchange, refresh rotation, logout, and IAM admin lifecycle. The next risk from the original security review is deployment-specific:

```text
spoofed forwarded header -> attacker-controlled rate-limit bucket
```

The infra design already says source IP should come from API Gateway request context. This slice implements and tests that boundary before broader AWS/KMS work.

## In Scope

### Trusted Source Identity Helper

Add a request-context source identity helper for rate limiting and audit source fields.

Target behavior:

```text
API Gateway v2 request context sourceIp -> trusted source IP
API Gateway v1 request context identity sourceIp -> trusted source IP if present
missing request context -> unknown source
x-forwarded-for -> ignored in api-gateway mode
x-real-ip -> ignored in api-gateway mode
```

Suggested code shape:

```text
packages/functions/auth/src/ratelimit/source.rs
packages/functions/auth/src/ratelimit/middleware.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/oauth/token.rs
```

The exact module name can differ, but the public route handlers should not each hand-roll source extraction.

### Rate-Limit Call-Site Cutover

Cut current rate-limited routes over to the trusted source helper.

Current known call sites to update:

```text
/authorize route middleware
/token
/password/register
/password/verify
/password/login
/password/forgot
/password/reset
legacy /:provider/callback password/code branches while still mounted
```

Required behavior:

- When API Gateway request context contains source IP, rate-limit keys use that IP.
- Spoofed `x-forwarded-for` and `x-real-ip` do not change rate-limit keys.
- Existing email/token digest components stay in password-flow rate-limit identifiers.
- Existing client ID priority for `/token` is preserved where a client ID is present.
- Missing source context falls back to `unknown` or an equivalent stable safe bucket.

Legacy hosted provider branches are still mounted until the legacy-removal slice, so any rate limits they still enforce must also stop trusting forwarded headers.

### Request Context Tests

Add focused tests that construct requests with Lambda/API Gateway request context extensions.

Test cases:

- API Gateway v2 source IP is used when present.
- Spoofed `x-forwarded-for` is ignored when API Gateway source IP exists.
- Spoofed `x-real-ip` is ignored when API Gateway source IP exists.
- Missing request context does not read forwarded headers in `api-gateway` mode.
- Password route rate-limit keys include the normalized email digest plus trusted source.
- Password route rate-limit keys do not contain raw email, password, token, or forwarded header values.
- `/authorize` rate-limit keys use `client_id + trusted source` where available.
- `/token` keeps client-ID based limiting when a client ID is provided.

Existing tests named around `extract_client_ip` should be rewritten so the target behavior is explicit. Tests that currently expect forwarded-header trust in API Gateway mode should be removed or inverted.

### Audit Source Alignment

Where audit events include a source identity in this slice, source must come from the same trusted request-context path used by rate limits.

In scope only where the current audit code already has a source field or simple request context access. Do not turn this slice into a full audit event redesign.

Required invariant:

```text
audit source never uses x-forwarded-for or x-real-ip in API Gateway mode
```

### API Route Shape Validation

Add static or unit-level validation around the SST API route shape.

Required checks:

- `$default` uses the public auth Lambda.
- `$default` does not require IAM.
- `GET /_admin/users/{subject}` uses the admin Lambda and IAM auth.
- `POST /_admin/users/{subject}/disable` uses the admin Lambda and IAM auth.
- `POST /_admin/users/{subject}/revoke-sessions` uses the admin Lambda and IAM auth.
- `POST /_admin/users/{subject}/delete` uses the admin Lambda and IAM auth.
- The admin Lambda environment does not receive Resend keys, Google client secrets, Apple private keys, or local JWT signing private keys by default.
- The public auth Lambda keeps the config required for public OAuth/provider/password flows.

If SST resources are difficult to assert directly, extract route definitions into a small typed structure in `infra/api.ts` or a sibling module so tests can validate the route metadata without deploying.

### AWS Dev Smoke Checklist

Add a short manual validation note for the first AWS dev deployment after this slice.

The checklist should cover:

```text
unsigned admin request -> rejected by API Gateway
SigV4 admin request with allowed IAM principal -> reaches admin Lambda
public OAuth route -> does not require IAM
spoofed x-forwarded-for on public route -> does not affect rate-limit identity
API Gateway request context sourceIp is visible to Lambda
```

This slice may add the checklist as documentation only. A live AWS deployment is useful but not required for local completion unless explicitly requested.

## Out Of Scope

- Customer managed DynamoDB KMS key.
- KMS ES256 token signing.
- Secrets Manager migration.
- Full IAM least-privilege policy reduction.
- WAF.
- Custom domains.
- Production CORS configuration.
- Access-log retention configurability.
- Full audit event taxonomy redesign.
- Removing legacy hosted UI/provider routes.
- Live AWS deployment as a required local test.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree where practical.

Target modules:

```text
infra/api.ts
infra/api.test.ts or scripts/type-level infra test equivalent
packages/functions/auth/src/ratelimit/middleware.rs
packages/functions/auth/src/ratelimit/source.rs if useful
packages/functions/auth/src/routes.rs
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/tests/runtime_route_slice.rs
packages/functions/auth/tests/rate_limit_source_slice.rs
```

Avoid broad refactors of unrelated rate-limit storage. The goal is trusted source selection and route-shape validation, not a new rate-limit engine.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing pure tests for API Gateway request-context source IP extraction.
2. Add failing tests proving forwarded headers are ignored in API Gateway mode.
3. Implement the trusted source helper.
4. Update `/authorize` rate-limit middleware to use request context.
5. Update `/token` rate limiting to use request context while preserving client ID priority.
6. Update password route rate limiting to use request context and retain email/token digest components.
7. Update legacy provider callback rate limits while those routes remain mounted.
8. Add route-level tests proving spoofed forwarded headers do not change stored rate-limit keys.
9. Add or update audit source tests where current audit code exposes source.
10. Add static validation for `infra/api.ts` route definitions and admin Lambda environment shape.
11. Add a short AWS dev smoke checklist if no existing doc owns it.
12. Run focused rate-limit tests, full Rust tests, admin crate check, `npm run typecheck`, setup tests, and any new infra tests.

## Tests

### Source Identity Tests

- API Gateway v2 request-context source IP is extracted.
- API Gateway v1 request-context source IP is extracted if supported by `lambda_http`.
- `x-forwarded-for` is ignored in API Gateway mode.
- `x-real-ip` is ignored in API Gateway mode.
- Missing request context yields `None` or `unknown`, not forwarded-header values.
- Source helper tests do not require a live AWS deployment.

### Rate-Limit Tests

- `/authorize` rate-limit key changes with request-context source IP, not forwarded headers.
- `/authorize` rate-limit key includes client ID when present.
- `/token` rate-limit key uses client ID when provided.
- Password registration/login reset route identifiers include trusted source and HMAC email/token digest.
- Password rate-limit keys do not include raw email, raw password, reset token, verification token, or spoofed header values.
- Existing 429 behavior and response shape remain unchanged.

### Infra Tests

- Public `$default` route points to the public auth Lambda.
- Public `$default` route does not have IAM auth.
- Every `/_admin/*` lifecycle route points to the admin Lambda.
- Every `/_admin/*` lifecycle route has IAM auth.
- Admin Lambda environment omits provider/email/signing secrets by default.
- Public auth Lambda environment still includes public auth runtime settings.

### Regression Tests

- Public admin bootstrap remains unmounted.
- Custom admin API keys still do not authenticate admin lifecycle routes.
- Password, token, refresh, Google, Apple, and admin lifecycle tests continue to pass.

## Acceptance Criteria

- API Gateway mode no longer trusts `x-forwarded-for`.
- API Gateway mode no longer trusts `x-real-ip`.
- Rate-limit source identity comes from Lambda/API Gateway request context where available.
- Password-flow rate-limit identifiers still combine trusted source with HMAC email/token digest where designed.
- `/token` retains client-ID based rate limiting where a client ID is present.
- Audit source identity, where touched, uses the same trusted source path.
- Admin lifecycle routes remain explicit IAM routes to the admin Lambda.
- Public auth routes remain public OAuth/OIDC routes.
- Tests cover spoofed forwarded-header attempts.
- No KMS, WAF, custom domain, or legacy-removal work is mixed into this slice.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml --test rate_limit_source_slice
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/admin/Cargo.toml
npm run typecheck
npm run test:setup
```

If an infra route test script is added:

```text
npm run test:infra
```

AWS dev validation after deployment:

```text
send public request with spoofed x-forwarded-for -> rate-limit source remains API Gateway sourceIp
send unsigned admin request -> API Gateway rejects it
send SigV4 admin request with allowed IAM principal -> admin Lambda handles it
send public OAuth request -> no IAM required
confirm Lambda logs do not print tokens, codes, passwords, or private keys
```

## Next Slice

After this slice, implement a separate AWS hardening slice for infrastructure permissions and storage encryption.

That follow-up should cover optional customer managed DynamoDB KMS, least-privilege DynamoDB permissions for public/admin Lambdas, CloudWatch retention configurability, and any required SST outputs or operator IAM policy examples. KMS ES256 signing can be its own slice if it is too large to include there.
