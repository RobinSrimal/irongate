# Counter Fault And Race Control

Source: [packages/functions/auth/tests/support/rate_control.rs](../../../../../packages/functions/auth/tests/support/rate_control.rs).

## What

A test-only adapter wrapper for bounded counter faults, read gates and protected-read observation.

## Why

Deterministic races and uncertain outcomes must exercise the backend's real conditional writes.

## How

Delegates storage operations to memory or DynamoDB Local. Matching counter dimensions can fail reads,
fail before a write, lose the response after a successful real CAS, exhaust conflict retries or gate
the first concurrent readers with a bounded barrier. Observation counts non-counter reads for route
admission tests. The wrapper does not invent successful CAS or transaction semantics; conflict-only
injection tests retry bounds, while race cases delegate conflicting writes to the actual backend.
