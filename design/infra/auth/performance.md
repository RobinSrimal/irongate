# Infra Performance

Target code: `infra/auth/api.ts`, Lambda runtime config, and auth runtime initialization.

## Owns

- Lambda sizing guidance.
- Client reuse requirements.
- External HTTP timeout/caching expectations.
- Load and cold-start validation targets.

## Runtime Reuse

The Rust Lambda should reuse clients across warm invocations:

```text
DynamoDB client
Resend HTTP client
Google/Apple HTTP client
JWKS cache
signing key or KMS key metadata cache
```

The current global DynamoDB client pattern is a good direction. The rewrite should extend that pattern to outbound HTTP clients and provider metadata.

## Outbound HTTP

External calls need explicit timeouts:

```text
Resend send email
Google token/JWKS/userinfo
Apple token/JWKS
```

JWKS should be cached by issuer and key ID with a bounded TTL. Cache misses must not create unbounded latency or repeated provider fetches under load.

## Lambda Sizing

Initial target:

```text
architecture = arm64
public auth Lambda memory = 256 MB
admin Lambda memory = 128 MB
timeout = 30 seconds
```

The public auth Lambda keeps the larger default because it handles Argon2 password verification,
token signing, DynamoDB SDK work, email delivery, and Google/Apple callback exchanges. The admin
Lambda is lower traffic and mostly runs account lifecycle DynamoDB operations, so `128 MB` is a
reasonable starting point.

Benchmark public auth:

```text
256 MB
512 MB
```

Benchmark admin if lifecycle routes become latency-sensitive:

```text
128 MB
256 MB
```

Measure:

- Cold start latency.
- `/authorize` latency.
- Password login latency.
- `/token` authorization-code exchange latency.
- Refresh rotation latency.
- Google/Apple callback latency.
- Admin account lookup, disable, enable, delete, and session revocation latency.

## Load Tests

Before production confidence, run focused load tests for:

- `/authorize`
- Password login.
- `/token` authorization-code exchange.
- Refresh token rotation.
- Email verification consume.

## Security Invariants

- Performance logging must not include tokens, codes, passwords, reset links, provider secrets, or private keys.
- Provider timeouts fail safely and do not mark users verified or issue tokens.
- Load testing should validate DynamoDB access patterns remain exact-key or bounded-query.
