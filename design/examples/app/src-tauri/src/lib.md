# Native Shell

Source: [packages/examples/app/src-tauri/src/lib.rs](../../../../../packages/examples/app/src-tauri/src/lib.rs).

## What

Tauri builder, auth command registration and tray integration.

## Why

The UI and native auth commands meet at one explicit invocation boundary.

## How

Registers login/refresh/logout/status handlers, opener and a tray menu with show/quit actions. Uses the bundled desktop logo and retains the tray handle. The mobile entrypoint attribute does not supply mobile-specific auth flows.
