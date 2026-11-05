# Password Registration Tests

Source: [packages/functions/auth/tests/password_registration.rs](../../../../packages/functions/auth/tests/password_registration.rs).

## What

Password policy/hash, email normalization/rendering, email-only registration and proof completion.

## Why

A pending signup cannot install a password before email proof, and completion cannot be replayed
to change credentials. Email delivery is not part of the credential transaction.

## How

Uses a captured mailbox and memory storage to inspect pending records, proof bindings and completed
credentials. Registration/resends retain one ID without a hash, hash timestamp, subject, account or
password identity, and return no auth tokens. Completion installs the proof holder's chosen hash
and subject/identity, clears the binding and returns no tokens.

Policy rejection leaves the proof usable for a valid completion. Replaying the consumed link or a
sibling link cannot replace the resulting hash. Registration also rejects already-verified users,
including after account disable. Invalid-proof cases cover absence, purpose, logical expiry and
registration-ID mismatch or a missing proof binding, plus missing/ineligible pending users, missing
pending bindings and mismatched email digest; they assert unchanged password state and no account creation.

Delivery failure leaves the pending instance available for a fresh link and later completion.
Policy/hash primitives, deterministic normalization, HTML escaping, token URL encoding and Resend
payload secret separation are checked without real mail. These are memory-backed domain tests,
not evidence of DynamoDB atomicity or live delivery; see the
[auth test guide](../../../../packages/functions/auth/tests/README.md) for integration execution.
