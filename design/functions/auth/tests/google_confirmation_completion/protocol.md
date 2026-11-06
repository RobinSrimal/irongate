# Confirmation OAuth Continuation

Source: [packages/functions/auth/tests/google_confirmation_completion/protocol.rs](../../../../../packages/functions/auth/tests/google_confirmation_completion/protocol.rs).

## What

Actual internal delivery/completion followed by OAuth token exchange, plus redirect encoding cases.

## Why

A stored code is useful only if the caller receives its original protocol-bound continuation.

## How

Captures begin's generated email code, completes on a reconstructed store and extracts the returned
authorization code for `/token`. Stored scope/generation, access-token subject and ID-token nonce
are checked; independent wrong client/redirect/PKCE and changed-generation cases reject. These
replace the store runner's token-route test rather than duplicate it.

A separate case preserves fixed/duplicate query parameters, loopback port/path and encoded optional
state, including its absence, with exactly one newly appended code. The stored redirect remains the
original without OAuth response parameters. No live provider/mailbox or hosted transport is tested.
