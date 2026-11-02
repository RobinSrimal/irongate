# App OAuth Helpers

Source: [packages/examples/app/src/auth/oauth.ts](../../../../../packages/examples/app/src/auth/oauth.ts).

## What

Pure TypeScript authorize URL and continuation parsing helpers.

## Why

Protocol-shaped inputs can be unit-tested independently of Tauri.

## How

Builds code/PKCE requests with provider/state/nonce and recognizes password/Google/Apple continuation paths. The live native flow also implements its requests in Rust; these helpers are not the native transport.
