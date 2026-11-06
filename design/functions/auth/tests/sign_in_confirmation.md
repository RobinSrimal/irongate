# Confirmation Contract Runner

Source: [packages/functions/auth/tests/sign_in_confirmation.rs](../../../../packages/functions/auth/tests/sign_in_confirmation.rs).

## What

Memory-backed durable Google confirmation contracts and a real OAuth token-route check.

## Why

Proof consumption must preserve existing token-exchange security, not just produce a stored row.

## How

Instantiates the shared confirmation suite with `TestStorage`, reusing provider and transaction-gate
fixtures. Successful confirmation produces a code exchanged through the actual router; access-token
subject and ID-token nonce are checked. Independent wrong PKCE/client/redirect and changed-generation
cases reject without tokens. This does not add or test a public confirmation endpoint.
