# Transaction Gate

Source: [packages/functions/auth/tests/support/transaction_gate.rs](../../../../../packages/functions/auth/tests/support/transaction_gate.rs).

## What

A test-only delegating StorageAdapter that pauses before one selected transaction or exact-key read.

## Why

Stale-writer tests need deterministic commit ordering while preserving real database conditions
and rollback behavior.

## How

Forwards other operations unchanged. The first matching operation sends its prepared transaction
(or an empty list for a read) and waits for a one-shot action. Commit delegates to the real backend;
failure before dispatch leaves it untouched; lost-response simulation delegates a real successful
commit then returns a fixed operational error. No conditional failure or success is fabricated,
no transaction is changed and no automatic retry occurs.

Gate arrival has a five-second bound and release a ten-second bound. Closure/cancellation produces
fixed test errors. Joined futures keep cleanup within the fixture. These controlled faults test
outcome handling, not production network reliability.
