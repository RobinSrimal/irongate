# Accounts

## What

Persisted generated subjects and lifecycle status in
[records](../src/store/records.md), managed by [AuthStore](../src/store/mod.md).

## Why

An account ID is independent of mutable email or external provider claims.

## How

Account/identity/index creation uses a transaction. Lifecycle writes compare the previously read
record, and deleted accounts are terminal. Account records retain subject, status, created_at and
optional disabled_at/deleted_at. There is no separate accounts.rs module or credential generation.
See [lifecycle](../core/account-lifecycle.md) for multi-step cleanup and concurrency limitations.
