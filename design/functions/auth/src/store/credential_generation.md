# Credential Generation Guards

Source: [packages/functions/auth/src/store/credential_generation.rs](../../../../../packages/functions/auth/src/store/credential_generation.rs).

## What

Active-account lookups and complete-value transaction conditions for credential operations.

## Why

Recovery invalidation must reject stale credentials and stale writers without enumerating tokens.

## How

Lookups require matching embedded subject and active status, optionally at an exact generation.
The shared equality helper serializes a complete validated record; the account guard contributes
a ConditionCheck to the caller's transaction. Missing/malformed generations are storage errors, not
generation zero. These helpers do not increment counters or hold a lock across JWT signing.
