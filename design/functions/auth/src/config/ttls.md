# Token Lifetimes

Source: [packages/functions/auth/src/config/ttls.rs](../../../../../packages/functions/auth/src/config/ttls.rs).

## What

Validated lifetimes for access/ID/refresh tokens, codes, sessions, provider state and email proofs.

## Why

Runtime expiration is separate from DynamoDB cleanup.

## How

Defaults are 3600-second access/ID tokens, 30-day refresh tokens, 300-second codes, 600-second sessions/state and 900-second email proofs. Validation requires positive values and enforces relative lifetime constraints.
