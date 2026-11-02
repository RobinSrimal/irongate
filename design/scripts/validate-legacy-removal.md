# Removed-Surface Checks

Source: [scripts/validate-legacy-removal.mjs](../../scripts/validate-legacy-removal.mjs).

## What

Static checks against obsolete module trees and public control-plane/storage paths.

## Why

Removed bootstrap, hosted UI and raw-token storage paths must not silently return.

## How

Checks forbidden source paths and patterns in the router, startup, token and infrastructure files. Passing establishes only those structural exclusions, not full security correctness.
