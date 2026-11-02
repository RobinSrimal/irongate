# Audit Persistence

Source: [packages/functions/auth/src/audit.rs](../../../../packages/functions/auth/src/audit.rs).

## What

AuditEvent construction and mode-gated event recording.

## Why

Auth/admin actions have a shared event shape and opt-out switch.

## How

Enabled events are serialized under the audit partition using the storage adapter with no expiry. Despite the cloudwatch mode name, these are DynamoDB writes. None skips persistence; ordinary Lambda tracing is separate, and call sites may ignore audit-write failures.
