# Rate Limits

Target code: `packages/functions/auth/src/store/rate_limits.rs`

## Owns

- Persisted rate-limit counters.
- Atomic counter updates.
- Expiry for rate-limit windows.

## Target Behavior

Rate limits should be keyed by the strongest available stable identifier:

```text
client_id when authenticated or declared
trusted source IP from API Gateway context
email digest for registration, login, verification, and reset attempts
```

Recommended composite identifiers:

```text
password registration: email_digest + source_ip
password login: email_digest + source_ip
email verification: verification_lookup_digest + source_ip
password reset request: email_digest + source_ip
password reset completion: reset_lookup_digest + source_ip
token endpoint: client_id + source_ip before auth validation; confidential clients may also use authenticated client identity
authorize endpoint: client_id + source_ip
oauth revoke endpoint: client_id + source_ip
provider authorize endpoint: provider + authorize_session_lookup_digest + source_ip
```

## Security Invariants

- Do not trust spoofable forwarded headers as source IP.
- Source IP must come from API Gateway/Lambda request context.
- Counter updates should be atomic.
- Counters should expire automatically.
- Limiter storage failures for sensitive public endpoints must fail safely instead of allowing requests.
- Rate-limit errors must not leak whether an email exists.
- Raw email addresses, passwords, authorize session keys, authorization codes, refresh tokens, reset tokens, and provider state values must not appear in rate-limit keys.

## Security Scan Coverage

This addresses the forwarded-header trust finding by removing `x-forwarded-for` and `x-real-ip` from the trusted rate-limit identity path in API Gateway mode.
