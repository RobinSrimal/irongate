# Persistent Rate Limits

Source: [packages/functions/auth/src/store/rate_limits.rs](../../../../../packages/functions/auth/src/store/rate_limits.rs).

## What

Typed source-first admission, fixed-window counters and private identifier construction.

## Why

Limits must coordinate across Lambda instances without allowing one changing identity component to
reset every budget.

## How

`RateLimitIdentity` supplies lookup secret, trusted source and a typed email/client/provider subject.
Canonical source and secondary identities use purpose-separated HMAC labels; compound inputs use
JSON tuples, and unknown clients use structural None. Invalid emails do not allocate email rows.
Policy/identity validation precedes counter I/O. Missing/invalid enabled policies fail closed;
explicit disable bypasses all counter access.

`ProviderEmailSend` uses the same independent normalized-email dimension as password admission.
Changing client/session or source does not change the email bucket; changing email does not reset
the source bucket. No new counter implementation or secret is required.

Charges the source bucket before touching the secondary counter and never refunds a prior charge.
Each bucket uses consistent reads, at most eight conditional writes and twice-window cleanup TTL.
Logical expiry does not depend on TTL deletion. Exhaustion returns 429 with remaining-window retry;
CAS exhaustion returns 429/one second; corrupt counters and operational failures return generic 503.
Diagnostics include fixed endpoint/operation categories only. `check_rate_limit_at` takes an explicit
evaluation time for shared backend contracts; the ordinary entry point uses UTC now.

See the [policy table and trade-offs](../../store/rate-limits.md).
