# Tauri Application

Source: [packages/examples/app/src-tauri/tauri.conf.json](../../../../packages/examples/app/src-tauri/tauri.conf.json).

## What

Example identity, frontend build commands, window sizing and bundle icons.

## Why

The native shell needs explicit build and packaging configuration.

## How

Uses Vite on port 1420 and built dist assets. The current CSP is null, not a hardened policy. Bundle identifiers are example configuration, and binary icon assets are excluded from design mirroring.
