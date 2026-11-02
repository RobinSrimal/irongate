# App Test Compilation

Source: [packages/examples/app/tsconfig.test.json](../../../packages/examples/app/tsconfig.test.json).

## What

NodeNext output for frontend auth helpers and Node tests.

## Why

Tests can run without starting Tauri or accessing the OS keychain.

## How

Extends root configuration and emits src/auth plus tests into dist with ES2022 and Node types. It is separate from the no-emit frontend configuration.
