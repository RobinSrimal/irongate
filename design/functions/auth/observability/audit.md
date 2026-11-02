# Audit Records

## What

Security-event records and the `cloudwatch | none` enablement setting. The implementation is
[src/audit.rs](../src/audit.md), reached through AuthStore from public and admin handlers.

## Why

Events identify auth/lifecycle actions without storing raw passwords or bearer values in the event
references. Token references are digests; the event type also allows subject, client, IP and detail.

## How

The current enabled path writes serialized AuditEvent records to the AuthTable `audit` partition
without TTL. Despite its name, `cloudwatch` does not emit structured CloudWatch audit events. Several
callers ignore persistence errors. `none` suppresses these writes without disabling ordinary Lambda
tracing or API Gateway logs.

CloudWatch retention therefore does not expire these DynamoDB audit records. The current event
shape is not a strict allowlisted emitter or a delivery guarantee; documentation must not claim
otherwise. See [mode parsing](../src/config/audit.md) and [infrastructure logs](../../../infra/auth/api.md).
