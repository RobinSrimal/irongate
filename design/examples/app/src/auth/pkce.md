# App PKCE Helpers

Source: [packages/examples/app/src/auth/pkce.ts](../../../../../packages/examples/app/src/auth/pkce.ts).

## What

Web Crypto verifier/challenge and random-string utilities.

## Why

Frontend protocol helper tests need deterministic checks against standard SHA-256 encoding.

## How

Encodes without base64url padding and produces 64-character verifiers. Byte-to-alphabet modulo mapping is biased for the 66-character verifier alphabet; native runtime randomness is implemented separately in Rust.
