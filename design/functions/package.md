# Function Workspace

Source: [packages/functions/package.json](../../packages/functions/package.json).

## What

The private npm workspace marker for the Rust function packages.

## Why

Keeps the repository workspace layout explicit without pretending npm builds the Rust handlers.

## How

Declares the functions workspace package; auth/admin dependency and build configuration live in their Cargo manifests.
