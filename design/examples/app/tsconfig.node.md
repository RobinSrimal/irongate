# Vite Type Checking

Source: [packages/examples/app/tsconfig.node.json](../../../packages/examples/app/tsconfig.node.json).

## What

Composite TypeScript settings for vite.config.ts.

## Why

Build-time Node configuration is distinct from WebView source.

## How

Checks the Vite config with ESNext modules and bundler resolution; it does not include browser auth state or native Rust.
