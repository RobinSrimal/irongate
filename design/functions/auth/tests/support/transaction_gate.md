# Transaction Gate

Source: [packages/functions/auth/tests/support/transaction_gate.rs](../../../../../packages/functions/auth/tests/support/transaction_gate.rs).

## What

A test-only delegating StorageAdapter that pauses immediately before a transaction.

## Why

Stale-writer tests need deterministic commit ordering while preserving real database conditions
and rollback behavior.

## How

Forwards reads and nontransaction writes unchanged. On transact, sends a copy of the prepared
operations to a controller and waits for a one-shot release before delegating the original operations
to the underlying backend. Gate arrival has a five-second bound and release a ten-second bound.
Closure/cancellation produces fixed test errors; the helper never fabricates a storage success or
conditional failure, retries, or changes the transaction. Tests use joined futures so unwinding
drops both sides and fixture cleanup can still run.
