# Integration Fixtures

Source: [packages/functions/auth/tests/support/mod.rs](../../../../../packages/functions/auth/tests/support/mod.rs).

## What

Shared TestStorage and NoopEmailSender implementations for cross-module integration tests.
Exports the shared email-first [registration fixture](registration.md) for verified password accounts.

## Why

Auth routes and stores can run locally without deployed tables, email or credentials.

## How

TestStorage uses a locked BTreeMap with expiry, prefix queries, CAS and condition-checked transactions.
Put replaces the complete value and TTL; there is no field-merging operation. It still treats expired
items as absent for conditions, whereas DynamoDB conditions see physical items, and its pagination
semantics differ. Those differences limit what passing tests prove; this fixture is not a DynamoDB
contract certification. NoopEmailSender returns a local delivery ID without sending mail.

Exports [audit fixtures](audit.md) for explicit capture/failure injection into public/admin state;
those use the production serializer/write path without deployment or database audit persistence.

Registration fixtures use the current domain registration/completion contract and a captured
mailbox. They do not reinstate removed store-only credential activation helpers.
