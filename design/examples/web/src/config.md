# Web Configuration

Source: [packages/examples/web/src/config.ts](../../../../packages/examples/web/src/config.ts).

## What

Issuer/client/base-URL settings, requested scopes and provider-button enablement.

## Why

Example configuration remains separate from the auth core.

## How

Reads Worker bindings, defaults issuer to a reserved example domain and client to web, and infers web origin from the incoming request unless WEB_BASE_URL is set. Provider flags require the string true. This loader does not enforce an origin allowlist.
