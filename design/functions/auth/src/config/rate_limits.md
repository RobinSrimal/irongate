# Endpoint Admission Policies

Source: [packages/functions/auth/src/config/rate_limits.rs](../../../../../packages/functions/auth/src/config/rate_limits.rs).

## What

Endpoint names, bounded rate-limit parameters and the enabled policy map.

## Why

Each endpoint needs explicit source/secondary dimensions; a missing policy must not silently admit.

## How

`RateLimitPolicy` holds a source limit and an optional secondary limit. Endpoint classification requires
secondary limits for email-bearing password routes, OAuth and provider start, but none for proof-only
verify/reset. Validation bounds counts to 1..=1,000,000 and windows to 1..=86,400 seconds. Defaults are
enabled with 60-second windows; the [rate-limit overview](../../store/rate-limits.md) lists thresholds.
Callers configure this Rust map; there are no separate stage variables or limiter secrets.
