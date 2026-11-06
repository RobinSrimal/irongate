# Shared Confirmation Contracts

Source: [packages/functions/auth/tests/support/sign_in_confirmation.rs](../../../../../packages/functions/auth/tests/support/sign_in_confirmation.rs).

## What

One Google confirmation fixture and contract set used by memory and DynamoDB Local runners.

## Why

Durability, expiry and proof binding must be observable across store instances and both backends.

## How

Uses actual HMAC families, synthetic typed Google claims after the assumed upstream validation
boundary, real ownership/password/provider fixtures and an injected provider clock. Reconstructs
AuthStore for preparation/completion, checks pending-only creation and validates committed codes.
Fresh/password/Apple/Google owners exercise one shared plan; replay and handle collisions preserve
state. Wrong independent proofs, cross-request code reuse, malformed records, missing generation
and read failures fail closed without proof consumption.

Creation helpers return the store-confirmed expiry; contracts compare it with the original session
cap and persisted record. Controlled clocks check session-capped and own-lifetime expiry, prepare/finish/immediate-dispatch
boundaries and invalid durations. Actual deletion tests independently require reusable identity and
owner tombstones and never revive the old subject. [Races](sign_in_confirmation/races.md) supply
conditional-writer, lifecycle, competing-claim and uncertain-outcome checks. Fixtures do not send
mail, validate live Google tokens or claim cookie/CSRF/admission protection.
