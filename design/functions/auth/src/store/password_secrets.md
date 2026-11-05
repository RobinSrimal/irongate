# Password Proofs

Source: [packages/functions/auth/src/store/password_secrets.rs](../../../../../packages/functions/auth/src/store/password_secrets.rs).

## What

Registration-bound verification proof creation and indexed reset-proof lifecycle cleanup.

## Why

Issuing a proof must not race past completion or replacement of its pending registration. Reset
cleanup needs subject indexes, not a table scan.

## How

`create_email_verification` loads an eligible pending user and copies its `registration_id` into a
digest-keyed `verify_email` proof. One transaction checks the complete pending value and creates the
proof conditioned on absence, with matching logical expiry and DynamoDB `expiry`. The raw bearer
token is not stored. A changed pending snapshot rejects issuance.

Proof consumption belongs exclusively to [atomic registration](registration.md). Sibling proofs may
coexist for the same pending instance; completion makes them unusable without scanning/deleting
them all. TTL is cleanup, not authorization. There is no standalone verification-consume API.

Subject cleanup queries reset indexes and removes each proof/index pair together. Atomic reset
creation/completion belongs to [recovery](recovery.md); there is no independent reset-consume API.
