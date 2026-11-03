# Atomic Password Recovery

Source: [packages/functions/auth/src/store/recovery.rs](../../../../../packages/functions/auth/src/store/recovery.rs).

## What

Generation-bound reset proof creation, opaque preparation and atomic completion.

## Why

A consumed link without a changed password and a changed password with usable older credentials
are both unsafe partial outcomes.

## How

Creation guards complete active-account and eligible password-user snapshots while writing the
HMAC-keyed proof and subject index with aligned logical/physical expiry. The proof captures generation.
Preparation validates purpose, expiry, account generation and verified/non-deleted user binding,
without writes; its snapshots are private. Callers hash the replacement before finish.

Finish checks counter overflow, builds four operations and rechecks expiry immediately before
dispatch using the recovery-only clock. One transaction deletes proof/index, conditionally replaces
the exact user hash/timestamps, and conditionally replaces the exact account with generation + 1.
Account/user stay persistent. No item has both a ConditionCheck and Put in the same transaction.

Confirmed conditional rejection returns false; backend/decoding/uncertain failures propagate.
The method does not retry using newer snapshots or revive inactive accounts. Other old credentials
remain physically stored until cleanup but fail their generation checks. This is not immediate
access-JWT revocation, email-verification atomicity or a whole-account deletion transaction.
