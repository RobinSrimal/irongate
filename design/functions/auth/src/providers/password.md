# Password Domain

Source: [packages/functions/auth/src/providers/password.rs](../../../../../packages/functions/auth/src/providers/password.rs).

## What

Registration, verification, login, reset request and reset completion operations.

## Why

Email verification is required before password login can issue an OAuth code.

## How

Registration stores a pending password hash and sends a proof link. Verification consumes the proof then creates/marks the identity. Login checks hash, verified user and active account, consumes a session and creates a code. Reset consumes its proof before replacing the hash in a separate write; there is no atomic recovery-generation invalidation.
