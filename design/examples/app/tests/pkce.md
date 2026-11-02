# App PKCE Tests

Source: [packages/examples/app/tests/pkce.test.ts](../../../../packages/examples/app/tests/pkce.test.ts).

## What

Tests of verifier/challenge and random-helper outputs.

## Why

Encoding and S256 behaviour can be checked without invoking providers.

## How

Uses Node Web Crypto and local assertions. This is not a randomness certification or a native keychain/device test.
