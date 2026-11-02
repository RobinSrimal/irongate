# Auth Observability

## What

Operational JSON logs, HTTP tracing and mode-gated audit persistence.

## Why

Operators need to distinguish request failures and lifecycle actions without treating raw auth
records as a dashboard data source.

## How

Public/admin startup configures tracing and SST configures CloudWatch log groups. Audit calls use
`src/audit.rs` and currently persist in DynamoDB even when the mode is named cloudwatch. There is no
observability source directory or custom metrics emitter.

- [Audit records](audit.md): actual persistence and failure behaviour.
- [Runtime metrics](metrics.md): existing operational signals.
- [Audit source mirror](../src/audit.md): event structure and storage boundary.
