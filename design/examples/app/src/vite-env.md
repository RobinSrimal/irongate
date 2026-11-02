# Vite Declarations

Source: [packages/examples/app/src/vite-env.d.ts](../../../../packages/examples/app/src/vite-env.d.ts).

## What

The Vite client type reference.

## Why

TypeScript needs ambient declarations for the frontend build environment.

## How

References vite/client; it contains no runtime configuration values or secret bindings.
