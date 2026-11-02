# Desktop Entrypoint

Source: [packages/examples/app/src-tauri/src/main.rs](../../../../../packages/examples/app/src-tauri/src/main.rs).

## What

The native executable entrypoint.

## Why

Executable setup delegates to the reusable Tauri library runner.

## How

Calls irongate_example_app_lib::run; the Windows release attribute suppresses the console window.
