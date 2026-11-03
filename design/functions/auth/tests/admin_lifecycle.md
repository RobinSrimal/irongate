# Lifecycle Tests

Source: [packages/functions/auth/tests/admin_lifecycle.rs](../../../../packages/functions/auth/tests/admin_lifecycle.rs).

## What

Account status transitions, indexed revocation, reset-proof cleanup and IAM route rejection.

## Why

Disable, enable and revoke-sessions have different user-visible contracts.

## How

Tests inject API Gateway IAM contexts, inspect sanitized responses, reject deleted reactivation and check audit mode none. Storage/audit assertions use TestStorage, not CloudWatch or deployed IAM.

Refresh fixtures bind generation zero to actual accounts. Reset-cleanup fixtures create eligible
verified password users and links before disabling them. Sanitized status responses must omit
credential generation as well as password/contact/refresh data. Real recovery/lifecycle races run
separately in the local recovery suite.
