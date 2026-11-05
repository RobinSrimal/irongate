# Lifecycle Tests

Source: [packages/functions/auth/tests/admin_lifecycle.rs](../../../../packages/functions/auth/tests/admin_lifecycle.rs).

## What

Account status transitions, indexed revocation, reset-proof cleanup and IAM route rejection.

## Why

Disable, enable and revoke-sessions have different user-visible contracts.

## How

Tests inject API Gateway IAM contexts, inspect sanitized responses and reject deleted reactivation.
State assertions use TestStorage and audit assertions use captured writer bytes, not CloudWatch or
deployed IAM.

Refresh fixtures bind generation zero to actual accounts created through email-first registration
and proof completion. Reset-cleanup fixtures derive the email digest for that same verified account
before issuing reset links, rather than attaching an unrelated password record. Sanitized status responses must omit
credential generation as well as password/contact/refresh data. Real recovery/lifecycle races run
separately in the local recovery suite.

The [audit child](admin_lifecycle/audit.md) exercises real emitted records, IAM actor fallback,
nonzero cleanup counts, both modes, failed requests and safe failure diagnostics independently of
tracing verbosity. Fixtures explicitly install captured emitters; no audit records are persisted.
