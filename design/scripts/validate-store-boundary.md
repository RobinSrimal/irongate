# Store Boundary Checks

Source: [scripts/validate-store-boundary.mjs](../../scripts/validate-store-boundary.mjs).

## What

Static enforcement that runtime routes/providers use AuthStore.

## Why

A publicly exported test backend contract must not become a route persistence API.

## How

Walks Rust boundary files, excludes inline unit-test modules and rejects raw StorageAdapter access/construction and scan calls. This is a source boundary check, not a DynamoDB access-pattern or race test.
