# Accounts

## What

Persisted generated subjects and lifecycle status in
[records](../src/store/records.md), managed by [AuthStore](../src/store/mod.md).

## Why

An account ID is independent of mutable email or external provider claims.

## How

Account/identity/index creation uses a transaction. Lifecycle writes compare the previously read
record, and deleted accounts are terminal. Account records retain subject, status, created_at,
optional disabled_at/deleted_at and required `credential_generation`. Every new account initializes
it to zero; atomic password recovery advances it with checked increment. Lifecycle replacements
preserve the counter and cannot overwrite a newer generation using a stale snapshot. It remains
internal and is omitted from sanitized admin responses. There is no separate accounts.rs module.
See [lifecycle](../core/account-lifecycle.md) for multi-step cleanup and concurrency limitations.
