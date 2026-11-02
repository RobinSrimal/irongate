# Crypto Modules

Source: [packages/functions/auth/src/crypto/mod.rs](../../../../../packages/functions/auth/src/crypto/mod.rs).

## What

Exports of encryption, HMAC, KMS, password, randomness and signing helpers.

## Why

Security primitives have explicit ownership outside routes.

## How

The module declares the six helper modules and re-exports selected cookie, random and signing types. HTTP policy and DynamoDB persistence remain in their respective layers.
