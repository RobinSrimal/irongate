# Password Proofs

Source: [packages/functions/auth/src/store/password_secrets.rs](../../../../../packages/functions/auth/src/store/password_secrets.rs).

## What

Verification/reset proof creation and consumption plus reset-by-subject cleanup.

## Why

Email proofs are short lived, one use and indexed for account lifecycle cleanup.

## How

Digest-keyed records have TTL and logical expiry. Reset creation/consumption includes a subject index transactionally. Proof consumption is separate from the later password or account update; subject cleanup queries reset indexes, not all verification records.
