# Auth Observability

## What

Operational JSON logs, HTTP tracing and mode-gated structured audit output.

## Why

Operators need to distinguish request failures and lifecycle actions without treating raw auth
records as a dashboard data source.

## How

Public/admin startup configures tracing and a separate synchronous AuditEmitter. SST preserves
existing CloudWatch log groups with finite retention and JSON/INFO settings. The emitter validates
typed events, writes stdout JSON and reports local failures safely on stderr. None skips audit,
not operational logging. Delivery is best effort; historical database rows remain untouched.

- [Audit events](audit.md): coverage, privacy, retention and failure behavior.
- [Runtime metrics](metrics.md): existing operational signals.
- [Audit source mirror](../src/audit.md): bounded output and independent diagnostic sinks.
