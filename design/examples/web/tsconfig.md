# Web TypeScript

Source: [packages/examples/web/tsconfig.json](../../../packages/examples/web/tsconfig.json).

## What

Compiler settings for Worker code, local adapter and tests.

## Why

One package supports Web APIs in Cloudflare and Node-based local verification.

## How

Extends root configuration, uses NodeNext/ES2022, includes DOM/WebWorker/Node types and emits code and declarations into dist. Includes src, scripts and tests.
