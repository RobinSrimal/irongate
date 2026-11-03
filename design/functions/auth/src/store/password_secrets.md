# Password Proofs

Source: [packages/functions/auth/src/store/password_secrets.rs](../../../../../packages/functions/auth/src/store/password_secrets.rs).

## What

Verification proof creation/consumption and indexed password-proof lifecycle cleanup.

## Why

Email proofs are short lived, one use and indexed for account lifecycle cleanup.

## How

Digest-keyed verification records have TTL and logical expiry. Verification consumption remains
separate from account verification. Subject cleanup queries reset indexes and removes each proof
and index together, not a table scan. Atomic reset creation/completion belongs to
[recovery](recovery.md); there is no independent reset-consume API.
