# Lifecycle Tests

Source: [packages/functions/auth/tests/admin_lifecycle.rs](../../../../packages/functions/auth/tests/admin_lifecycle.rs).

## What

Account status transitions, indexed revocation, reset-proof cleanup and IAM route rejection.

## Why

Disable, enable and revoke-sessions have different user-visible contracts.

## How

Tests inject API Gateway IAM contexts, inspect sanitized responses, reject deleted reactivation and check audit mode none. Storage/audit assertions use TestStorage, not CloudWatch or deployed IAM.
