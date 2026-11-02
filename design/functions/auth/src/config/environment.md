# Runtime Configuration

Source: [packages/functions/auth/src/config/environment.rs](../../../../../packages/functions/auth/src/config/environment.rs).

## What

Startup assembly of clients, HMAC secret, signer, provider settings, email, TTLs and lifecycle policy.

## Why

Invalid required configuration is rejected before requests are served.

## How

The environment loader resolves secret references, loads the client file and selects local ES256 or asynchronous KMS initialization. LookupSecret redacts Debug output and requires at least 32 bytes; test constructors avoid deployed services.
