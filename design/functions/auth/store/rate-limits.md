# Rate Limits

Implementation: [store/rate_limits.rs](../src/store/rate_limits.md),
[configuration](../src/config.md) and [source identity](../src/ratelimit/middleware.md).

## What And Why

Persisted fixed-window counters enforce configured endpoint budgets across warm Lambda instances.
Source identity comes from API Gateway request context, never forwarded request headers.

## How

Identifiers combine source IP with normalized email HMAC, client ID or provider/session digest where
the handler supplies one. Source-only endpoints use a source bucket; missing context becomes
`unknown`. Raw passwords and flow tokens are not used in these keys.

A conditional compare-and-set loop retries up to eight times. Exceeding the budget returns 429;
persistent contention also returns 429 with a short retry delay. Storage failures fail closed as
limiter-unavailable. Counter writes include cleanup TTL and the window is checked at runtime.
A disabled limiter or missing endpoint policy skips the check.

These are composite buckets, not independent source and account/client budgets. Changing one
component can select a different bucket. The existing concurrency tests hold the bucket fixed and
do not establish resistance to rotating emails, clients or sessions.
