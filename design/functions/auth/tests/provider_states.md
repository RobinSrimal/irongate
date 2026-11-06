# Provider State Contracts

Source: [packages/functions/auth/tests/provider_states.rs](../../../../packages/functions/auth/tests/provider_states.rs).

## What

Memory-backed runner for the shared provider-state contracts.

## Why

Proof rejection, replay and races must check actual stored outcomes, not just callback mocks.

## How

Supplies a fresh TestStorage to each case in [shared state tests](support/provider_states.md),
using the existing transaction gate for deterministic ordering and controlled failures. The same
cases run through the real adapter in [DynamoDB Local](dynamodb_storage/provider_states.md).
Unused gate helpers are allowed only on this runner's shared support module import.
