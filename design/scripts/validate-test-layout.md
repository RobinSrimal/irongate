# Test Layout Checks

Source: [scripts/validate-test-layout.mjs](../../scripts/validate-test-layout.mjs).

## What

Checks for domain-oriented integration test filenames and a test README.

## Why

Delivery-slice naming does not explain the contracts a test protects.

## How

Examines top-level auth integration .rs files and rejects numeric or _slice names. It does not check behaviour, coverage or the repository-wide line limit.
