# Window Capability

Source: [packages/examples/app/src-tauri/capabilities/default.json](../../../../../packages/examples/app/src-tauri/capabilities/default.json).

## What

Core and opener permissions for the main Tauri window.

## Why

Window/plugin access is declared in configuration.

## How

Grants core:default and opener:default to main and references the generated desktop schema. Custom command registration remains in src/lib.rs.
