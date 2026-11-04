# Admin Audit Tests

Source: [packages/functions/auth/tests/admin_lifecycle/audit.rs](../../../../../packages/functions/auth/tests/admin_lifecycle/audit.rs).

## What

Real-router audit destination, attribution, lifecycle counts and output-failure tests.

## Why

Admin outcome evidence must agree with persisted state and trusted actor provenance.

## How

Enabled/none reads compare output with no new audit rows. Lifecycle routes use actual password
registration/verification/reset fixtures and nonzero cleanup counts. Forged headers cannot supply
actor identity; IAM ARN/caller/user fallback and missing context are checked. Denied/missing/conflict
requests add no success records. A failed writer leaves disable committed with a safe independent
diagnostic. Uses memory storage and injected context, not deployed IAM or CloudWatch.
