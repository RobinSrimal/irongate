# Shared Counter Contract

Source: [packages/functions/auth/tests/support/rate_contract.rs](../../../../../packages/functions/auth/tests/support/rate_contract.rs).

## What

One set of admission cases instantiated against memory and disposable DynamoDB Local.

## Why

Both backends must enforce observable quota, race, failure and expiry behavior.

## How

Uses explicit limiter evaluation time and bounded endpoint queries. Cases cover exact window rollover
while records remain, concurrent source/email caps with the other dimension varying, source-first
accounting, failed or uncertain writes, eight-conflict exhaustion, malformed counters, invalid
policies/identities and private canonical keys. Disabled policies do no I/O; invalid emails charge
only source. Faults and gates wrap the real adapter under test. Time advances only for limiter
evaluation, not the database clock or TTL cleanup. The macro exposes the same six cases in both
test targets without claiming full backend equivalence.
