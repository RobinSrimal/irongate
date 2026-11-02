# Web Session Tests

Source: [packages/examples/web/tests/session.test.ts](../../../../packages/examples/web/tests/session.test.ts).

## What

Session-store CRUD and secure-cookie helper checks.

## Why

Storage substitution and cookie serialization underpin the BFF routes.

## How

Node tests exercise the in-memory contract and cookie properties. They do not test real Durable Object isolation, expiry cleanup or refresh races.
