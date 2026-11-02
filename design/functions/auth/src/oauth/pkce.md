# PKCE Helpers

Source: [packages/functions/auth/src/oauth/pkce.rs](../../../../../packages/functions/auth/src/oauth/pkce.rs).

## What

S256 challenge generation, verifier generation and comparison.

## Why

A code must be exchanged by the client holding its verifier.

## How

SHA-256 output is base64url without padding and compared using ConstantTimeEq. Verifiers use 32 random bytes. The comparison helper does not itself validate the RFC verifier length/character grammar; tests include the RFC challenge vector.
