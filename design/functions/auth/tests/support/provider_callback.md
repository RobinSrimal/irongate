# Provider Callback Faults

Source: [packages/functions/auth/tests/support/provider_callback.rs](../../../../../packages/functions/auth/tests/support/provider_callback.rs).

## What

A shared test-only storage wrapper for preparation/finish failures in Google and Apple HTTP callbacks.

## Why

Both routes must distinguish confirmed conditional failures from operational or uncertain outcomes
without leaking storage details. Faults must occur after real state/session consumption, not bypass it.

## How

`FaultStorage` delegates ordinary reads, writes, conditions and transactions to its caller-provided
adapter. It injects ConditionFailed, AlreadyExists, DynamoDB or TransactionConflict either at the
identity read or at a transaction containing a code Put. A counter exposes accidental retries.
Injected messages contain test-only sensitive-looking values; HTTP tests require exact safe bodies.

`issues_code` selects issuance transactions for this wrapper and the unchanged transaction gate.
These faults test error translation, not atomicity; separate gated tests run actual commits, stale
conditions and lost responses over TestStorage, while shared Local contracts verify real DynamoDB.
