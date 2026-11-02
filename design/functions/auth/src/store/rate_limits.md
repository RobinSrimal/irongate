# Persistent Rate Limits

Source: [packages/functions/auth/src/store/rate_limits.rs](../../../../../packages/functions/auth/src/store/rate_limits.rs).

## What

Fixed-window request counters and identifier construction.

## Why

Limits must coordinate across Lambda instances.

## How

Counters use compare-and-set with up to eight attempts and physical expiry. Current identifiers combine source with email digest, client or provider/session; they are not independent source/account budgets. Contention returns rate-limited, backend errors unavailable. Disabled limits or absent endpoint policy bypass accounting.
