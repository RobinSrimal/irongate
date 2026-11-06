# Google Confirmation Delivery Contracts

Source: [packages/functions/auth/tests/google_confirmation_delivery.rs](../../../../packages/functions/auth/tests/google_confirmation_delivery.rs).

## What

Internal delivery/admission/failure tests using the real domain operation, typed store and a captured
email sender. No live Resend or Google requests are made.

## Why

Success must mean a usable, correctly bound pending challenge; unavailable responses must not
misrepresent committed state or remote acceptance.

## How

Reuses confirmation/provider fixtures, memory storage, the provider clock and transaction gates.
The captured generated code and returned handle complete on a reconstructed store only with the
original browser proof; wrong proofs and replay fail. Stored digests, pending-only creation,
session-capped/configured expiry and original client/redirect/PKCE/nonce/generation are inspected.

Low source/email policies exercise changing candidates, clients and sessions under one source,
then varying sources for one normalized address. Invalid/missing policies and counter outages
stop creation/delivery; explicit limiter disable retains its existing semantics. Ineligible
identity/session/browser inputs send nothing.

Gates order actual creation conflicts, pre-dispatch failure, lost responses after commit and expiry
before send. A captured-then-failing sender leaves pending state and charges without retry/refund;
a paused sender checks expiry during delivery. Fixed errors omit backend markers. Pure rendering
tests in `email/templates.rs` independently cover leading zeros, exact deadline and HTML escaping.

These tests do not certify browser enforcement, public code-attempt limits, real email delivery or
Google acceptance. Existing shared memory/DynamoDB Local contracts verify the unchanged store
transaction shape and the returned deadline against persisted logical/physical expiry.
