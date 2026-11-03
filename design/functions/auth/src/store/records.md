# Auth Records

Source: [packages/functions/auth/src/store/records.rs](../../../../../packages/functions/auth/src/store/records.rs).

## What

Serializable account, identity, password, proof, session, code and refresh records.

## Why

Typed records preserve the information needed for expiry, replay and lifecycle checks.

## How

Short-lived records carry expires_at independently of physical expiry. Account/identity statuses
encode lifecycle state; optional password/contact fields support tombstones. Accounts, reset proofs,
authorization codes, refresh tokens and refresh families require `credential_generation: u64`.
Missing fields fail deserialization, not default to zero. Recovery increments the account counter;
bound credentials retain their original generation. It is not a JWT claim or an admin response field.
