# Shared Provider State Tests

Source: [packages/functions/auth/tests/support/provider_states.rs](../../../../../packages/functions/auth/tests/support/provider_states.rs).

## What

Backend-independent fixtures and three contracts for typed provider-state creation/consumption.

## Why

Memory and DynamoDB must agree on proof admission and exact-value conditions, including ambiguous
failures that do not imply rollback.

## How

Fresh fixture state/session/browser digests use the actual HMAC families. A provider-only clock
checks exact expiry and the final pre-dispatch check without sleeps or TTL deletion. Cases cover
Google versus Apple proofs, malformed stored bindings, duplicate creation, single-use consumption
and rejection leaving the exact record usable.

The existing transaction gate pauses real deletes/reads. Tests order two consumers and a raw-value
replacement (including an unknown field), and distinguish pre-dispatch/read failures from a lost
response after actual commit. The stale consumer never retries or adopts the replacement. These
are storage contracts; HTTP cookie parsing and signed-token rejection run in callback tests.
