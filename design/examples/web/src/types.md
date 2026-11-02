# Web Contracts

Source: [packages/examples/web/src/types.ts](../../../../packages/examples/web/src/types.ts).

## What

Worker bindings, login/app session records and storage interfaces.

## Why

The example can use lightweight structural types in both Worker and Node tests.

## How

WebEnv describes production and injected local/test dependencies. Login records contain state, nonce, verifier and expiry; app records contain server-side tokens/userinfo. Having an expiry field alone does not enforce expiration.
