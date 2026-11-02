# Rate-Limit Source Helpers

Source: [packages/functions/auth/src/ratelimit/mod.rs](../../../../../packages/functions/auth/src/ratelimit/mod.rs).

## What

The rate-limit middleware module export.

## Why

Source attribution is separate from persistent counters.

## How

Exports middleware only. Bucket construction and compare-and-set accounting live in store/rate_limits.rs and are invoked by routes.
