# Memory Admission Contracts

Source: [packages/functions/auth/tests/rate_limits.rs](../../../../packages/functions/auth/tests/rate_limits.rs).

## What

Memory-backed shared counter contracts and sanitized diagnostic capture.

## Why

Fast tests cover admission behavior and prevent limiter diagnostics exposing request identities or
backend error text.

## How

Instantiates the [shared contract](support/rate_contract.md) with the integration TestStorage. A single
process-wide tracing capture also checks read/write failures produce fixed safe categories without
IP, email, digest or injected backend errors. It includes concurrent test events; this binary has no
other subscriber. Real conditional-write behavior is checked separately by DynamoDB Local.
