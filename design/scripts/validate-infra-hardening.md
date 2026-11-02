# Infrastructure Static Checks

Source: [scripts/validate-infra-hardening.mjs](../../scripts/validate-infra-hardening.mjs).

## What

Source-level guardrails for stage, IAM, signing, table, logging and example wiring.

## Why

Accidental configuration regressions can be caught without a deployment.

## How

Reads selected TypeScript files and operator-policy documentation, asserting expected patterns and forbidden broad permissions/config. These regex checks are structural checks, not synthesized-policy evaluation or live AWS/Cloudflare verification.
