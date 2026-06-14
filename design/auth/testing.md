# Auth Testing

Target code: auth crate tests plus integration tests around the Rust Lambda where useful.

## Owns

- Security regression test plan.
- Store behavior tests.
- Provider-flow tests.
- Config validation tests.

## Required Regression Tests

Security scan coverage:

- Registration with verification required does not issue an OAuth code.
- Login for an unverified password user fails.
- Public admin bootstrap route does not exist in the target core.
- Rate-limit identity ignores spoofed `x-forwarded-for` and `x-real-ip`.
- Verification/reset attempt updates preserve expiry.

Storage security:

- Authorization code key uses HMAC lookup digest, not raw code.
- Refresh token key uses HMAC lookup digest, not raw token.
- Verification/reset keys use HMAC lookup digest, not raw secret.
- Expired records are rejected before DynamoDB TTL deletion.
- Refresh rotation is atomic and detects reuse.

Provider behavior:

- Google identity uses issuer plus subject, not email.
- Apple identity uses issuer plus subject, not email.
- Provider state is single-use.
- OIDC nonce is validated.

Email behavior:

- Missing `RESEND_API_KEY` fails startup.
- Missing `AUTH_EMAIL_FROM` fails startup.
- Resend delivery failure does not mark users verified.
- Password reset request does not reveal whether an email exists.

## Test Boundaries

Runtime uses Resend only. Tests may use a mock email sender internally, but the production configuration model should not expose a console or provider switch.

## AWS Validation

Before production confidence:

- Deploy to AWS dev account.
- Confirm API Gateway source IP is available in request context.
- Confirm spoofed forwarded headers do not affect rate-limit keys.
- Confirm DynamoDB TTL attributes are written on short-lived records.
- Confirm no raw bearer values appear in `pk` or `sk`.
- Run load tests for `/authorize`, password login, `/token`, refresh rotation, and email verification consume.
- Measure cold start and compare 256 MB vs 512 MB Lambda memory.
