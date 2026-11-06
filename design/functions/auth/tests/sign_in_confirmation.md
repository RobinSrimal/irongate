# Confirmation Contract Runner

Source: [packages/functions/auth/tests/sign_in_confirmation.rs](../../../../packages/functions/auth/tests/sign_in_confirmation.rs).

## What

Memory-backed durable Google confirmation contracts.

## Why

Proof admission and consumption must retain snapshots, expiry and atomicity across store instances.

## How

Instantiates the shared confirmation suite with `TestStorage`, reusing provider and transaction-gate
fixtures. Staged browser/code admission, expiry, proof binding, independent conditions and races
run against the same contract used by DynamoDB Local. The end-to-end token-route cases live in
[completion protocol tests](google_confirmation_completion/protocol.md), exercising actual internal
delivery and completion rather than a separate store-only shortcut. No public endpoint is exposed.
