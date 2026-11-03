# Local Recovery Races

Source: [packages/functions/auth/tests/dynamodb_storage/recovery_races.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/recovery_races.rs).

## What

Deterministically ordered recovery races through the real DynamoDB adapter.

## Why

Old password checks, codes and refresh sessions must not mint new-generation credentials or
overwrite a completed recovery through stale snapshots.

## How

Selected read/transaction gates order same-link and different-link resets, refresh in both orders,
password login including reset before its account read, code consumption and follow-up initial
refresh issuance, and reset-link creation before email delivery. Real conditions reject stale
writes without orphan tokens/indexes or partial TTL changes.

Disable/delete races run in both orders. Recovery cannot revive an inactive account; stale lifecycle
writes cannot roll generation back. A fresh explicit lifecycle attempt still succeeds after its
visible failure. Assertions inspect state and usable credentials, not transaction counts. No sleeps,
scans, fabricated conditional failures or deployed tables are used. Signing/response delivery and
multi-step lifecycle cleanup remain outside the reset transaction.
