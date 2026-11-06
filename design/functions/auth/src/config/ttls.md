# Token Lifetimes

Source: [packages/functions/auth/src/config/ttls.rs](../../../../../packages/functions/auth/src/config/ttls.rs).

## What

Validated lifetimes for access/ID/refresh tokens, codes, sessions, provider state and email proofs.

## Why

Runtime expiration is separate from DynamoDB cleanup.

## How

Defaults are 3600-second access/ID tokens, 30-day refresh tokens, 300-second codes, 600-second sessions/state and 900-second email proofs. Validation requires positive values and enforces relative lifetime constraints.

Internal Google confirmation delivery requests the provider-state lifetime; the store caps it by
the original authorize-session deadline. Sending returns that confirmed deadline, never a new
lifetime measured after delivery. No confirmation-specific TTL setting is introduced.
