# Startup Tests

Source: [packages/functions/auth/tests/startup_config.rs](../../../../packages/functions/auth/tests/startup_config.rs).

## What

Client registry and required/optional secret validation.

## Why

Invalid deployment configuration must fail before authentication requests run.

## How

Temporary client files, environment maps and generated test keys cover missing inputs, partial provider setup, client grants/redirects/PKCE and confidential secrets. These tests do not resolve real SST secrets.
