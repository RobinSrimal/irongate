# Auth Crate

Source: [packages/functions/auth/Cargo.toml](../../../packages/functions/auth/Cargo.toml).

## What

The irongate library and bootstrap binary manifest.

## Why

Pins the AWS SDK versions used by storage/signing while declaring the Rust HTTP and crypto stack.

## How

Uses Axum/lambda_http, Tokio, DynamoDB/KMS SDKs, reqwest with rustls, Argon2 and JWT/P-256 helpers. Release builds strip symbols, enable LTO and optimize size. Integration tests use tokio-test; lockfiles pin resolved dependencies.
