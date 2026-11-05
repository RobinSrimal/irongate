# Apple Ownership Races

Source: [packages/functions/auth/tests/support/provider_issuance/apple_races.rs](../../../../../../packages/functions/auth/tests/support/provider_issuance/apple_races.rs).

## What

Backend-neutral conditional-write, competing-claim and lifecycle race contracts for Apple ownership.

## Why

Prepared ownership cannot authorize a later stale account selection or partial credential write.
An operational error must not be confused with rollback.

## How

The existing gate pauses transactions before the supplied memory or DynamoDB adapter executes them.
Each of the six fresh/existing-owner conditions is independently invalidated, then exact-key
snapshots prove all other writes rolled back. Both commit orders race same/different new Apple
identities for one available email, verifying no losing account/index/code remains.

Password registration and Apple both prepare before either commits. When Apple wins, the losing
password completion leaves its pending user/proof untouched, installs no credential and cannot
reprepare against the active owner. When password wins, stale Apple issuance fails and only a fresh
sign-in can select the committed owner. New Apple attachment also races actual password recovery,
disable, delete and owner/index changes. Failure before dispatch preserves snapshots; deliberately
lost response after a real commit leaves a usable code and complete identity state. Tests do not
use deployed tables, substitute fake successful writes or promise immediate JWT revocation.
