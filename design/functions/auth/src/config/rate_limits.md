# Endpoint Admission Policies

Source: [packages/functions/auth/src/config/rate_limits.rs](../../../../../packages/functions/auth/src/config/rate_limits.rs).

## What

Endpoint names, bounded rate-limit parameters and the enabled policy map.

## Why

Each endpoint needs explicit source/secondary dimensions; a missing policy must not silently admit.

## How

`RateLimitPolicy` holds a source limit and an optional secondary limit. Endpoint classification requires
secondary limits for email-bearing password routes, internal provider email sending, OAuth and
provider start, but none for proof-only
verify/reset. Validation bounds counts to 1..=1,000,000 and windows to 1..=86,400 seconds. Defaults are
enabled with 60-second windows; the [rate-limit overview](../../store/rate-limits.md) lists thresholds.
Callers configure this Rust map; there are no separate stage variables or limiter secrets.

`ProviderEmailSend` uses the `provider_email_send` key, with 20 source and 3 normalized-email
requests per minute. It is an internal admission category, not a mounted route or verification-attempt
policy. Existing explicit disable behavior also applies to it.

Completion has two internal categories: `ProviderEmailAttempt` (`provider_email_attempt`) charges
5 source attempts/minute before any pending lookup, with no secondary dimension.
`ProviderEmailConfirm` (`provider_email_confirm`) charges 30 source and 5 normalized-email attempts
per minute after browser admission, before code verification. The second source charge is additional
conservative accounting, not a replacement for the pre-load ceiling. These are fixed windows, not
lifetime lockouts. No public route or generic passwordless-code endpoint is added.
