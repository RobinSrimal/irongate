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

`commit_and_hold_response` delegates the actual commit, reports completion to the test and holds
its successful response behind a separate bounded one-shot release. This distinguishes a delayed
success from a lost response and lets tests advance logical time only after real commit.

Gate arrival has a five-second bound and release a ten-second bound. Closure/cancellation produces
fixed test errors; the post-commit release has a five-second bound. Joined futures keep cleanup within the fixture. These controlled faults test
outcome handling, not production network reliability.
