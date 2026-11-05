# Memory Provider Issuance

Source: [packages/functions/auth/tests/provider_issuance.rs](../../../../packages/functions/auth/tests/provider_issuance.rs).

## What

Ordinary Cargo runner for eight shared provider identity/code transaction contracts.

## Why

Fast local tests exercise the same observable conditions, races, bindings and uncertain outcomes
as the opt-in DynamoDB Local runner without a separate copy of the cases.

## How

Each case receives a new `TestStorage` through `Arc<dyn StorageAdapter>`. The runner imports the
provider contract and bounded transaction gate; `support` also supplies the real email-first
registration fixture used to prove provider sign-in leaves an existing password account unchanged.
Only unused gate helpers receive a dead-code allowance. No Java, network service or live provider
is required. This runner alone does not establish DynamoDB conditional-write equivalence.
