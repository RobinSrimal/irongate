# Memory Registration Contracts

Source: [packages/functions/auth/tests/registration_contract.rs](../../../../packages/functions/auth/tests/registration_contract.rs).

## What

Ordinary Cargo runner for shared registration and email-ownership contracts.

## Why

Fast local checks exercise the same typed-store race, rejection, password and uncertain-outcome
contracts as the opt-in real DynamoDB adapter tests.

## How

Each case receives a fresh `TestStorage` through `Arc<dyn StorageAdapter>`. The runner includes the
shared registration and ownership contracts, bounded transaction gate and crate-root recovery
fixture for real login/reset checks. Ownership cases exercise policy and malformed-state rejection,
clocked retention boundaries, guarded deletion/reuse races and cleanup retry after failure or a
committed write whose response was lost. Both suites run unchanged in the Local target.
Unused recovery/gate helpers are allowed only on these imported modules. It does not require Java,
AWS credentials or a network service and does not establish DynamoDB equivalence on its own.
