# Memory Registration Contracts

Source: [packages/functions/auth/tests/registration_contract.rs](../../../../packages/functions/auth/tests/registration_contract.rs).

## What

Ordinary Cargo runner for the eight shared registration contracts.

## Why

Fast local checks exercise the same typed-store race, rejection, password and uncertain-outcome
contracts as the opt-in real DynamoDB adapter tests.

## How

Each case receives a fresh `TestStorage` through `Arc<dyn StorageAdapter>`. The runner includes the
shared contract, bounded transaction gate and crate-root recovery fixture for real login checks.
Unused recovery/gate helpers are allowed only on these imported modules. It does not require Java,
AWS credentials or a network service and does not establish DynamoDB equivalence on its own.
