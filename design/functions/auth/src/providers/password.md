# Password Domain

Source: [packages/functions/auth/src/providers/password.rs](../../../../../packages/functions/auth/src/providers/password.rs).

## What

Registration, verification, login, reset request and reset completion operations.

## Why

Email verification is required before password login can issue an OAuth code.

## How

Registration stores a pending password hash and sends a proof link. Verification consumes the proof
then creates/marks the identity. Login checks hash, verified user and active account, consumes a
session, then creates a generation-bound code guarded by both account and verified user snapshots.
A stale password check cannot acquire the new generation after recovery.

Forgot-password returns a generic response; eligible proof creation guards account/user snapshots
before mail is sent. Confirmed condition races return that response without sending mail; operational
failures propagate. Completion validates policy, prepares proof/account/user without consumption,
hashes and finishes one conditional recovery transaction. Confirmed rejection is generic invalid
reset; uncertain or operational failures are not reported as success or rollback. No reset response
issues tokens or logs in the user.
