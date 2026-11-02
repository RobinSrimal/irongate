# Integration Fixtures

Source: [packages/functions/auth/tests/support/mod.rs](../../../../../packages/functions/auth/tests/support/mod.rs).

## What

Shared TestStorage and NoopEmailSender implementations for cross-module integration tests.

## Why

Auth routes and stores can run locally without deployed tables, email or credentials.

## How

TestStorage uses a locked BTreeMap with expiry, prefix queries, CAS and condition-checked transactions. It currently treats expired items as absent for conditions and merges fields for Update, whereas DynamoDB conditions see physical items and its adapter replaces the JSON value. These differences limit what passing tests prove; this fixture is not a DynamoDB contract certification. NoopEmailSender returns a local delivery ID without sending mail.
