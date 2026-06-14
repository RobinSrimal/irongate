# Metrics

Target code: `packages/functions/auth/src/observability/metrics.rs`

## Owns

- Counters and dimensions safe for CloudWatch or a sanitized metrics table.

## Target Metrics

- Authorization starts.
- Login success by provider.
- Login failure by reason category.
- Token exchange success/failure.
- Refresh token reuse.
- Rate-limit exceeded.
- Provider latency.

## Security Invariants

- Metrics must not include raw email addresses, subjects, tokens, or IPs as high-cardinality dimensions.
- Metrics are sanitized projections, not a second view over `AuthTable`.
- Public or operator-facing identifiers should be coarse or hashed specifically for analytics.
- Metrics must not require scanning raw auth records.
