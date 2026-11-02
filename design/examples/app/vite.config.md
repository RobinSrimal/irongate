# Vite Development

Source: [packages/examples/app/vite.config.ts](../../../packages/examples/app/vite.config.ts).

## What

React plugin, fixed dev port and Tauri-aware HMR settings.

## Why

Tauri expects a predictable frontend endpoint and its Rust files must not trigger frontend reload loops.

## How

Uses port 1420 with strictPort, optional TAURI_DEV_HOST and HMR port 1421. Excludes src-tauri from Vite watching and keeps compiler output visible.
