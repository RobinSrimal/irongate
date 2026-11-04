# Structured Audit Logging

## What

The public and admin Lambdas emit ten typed security outcomes through their existing application
logs. [The emitter](../src/audit.md) is injected into application state, separately from AuthStore.
`AUTH_AUDIT_LOG_MODE=cloudwatch|none` defaults to `cloudwatch`; unknown runtime values fail startup.

## Why

Confirmed auth/lifecycle outcomes need bounded, inspectable records without a second persistence
mechanism or bearer-secret exposure. Logging failure must not undo a completed change or weaken a
replay rejection.

## How

CloudWatch mode synchronously attempts one compact JSON line on stdout before the handler returns.
It contains `record_type=auth_audit`, schema version 1, UUID, RFC3339 timestamp, INFO level and a
fixed event kind. [Typed variants](../src/audit/events.md) admit only bounded client/subject/operator
identifiers, validated HMAC token digests, numeric cleanup counts and typed revocation reasons.
There is no raw IP or free-form detail. Passwords, end-user emails, raw tokens/codes, headers,
cookies, bodies, provider payloads and underlying errors are excluded.

Admin actor attribution uses trusted API Gateway IAM authorizer context: user ARN, then nonempty
caller ID, then user ID. The target subject is separate. These identifiers and digests are
correlating data, not anonymous data; an IAM session name can identify an operator.

Existing event coverage is code exchange, refresh issuance/rotation/reuse/revocation and admin
read/disable/enable/delete/revoke-sessions. Initial refresh issuance is emitted after persistence;
rotation immediately after its transaction, before response signing. Unknown/foreign revocation,
rejected exchange and denied admin actions do not produce success events. Already-revoked tokens
can produce the typed idempotent result.

Validation, serialization, size and output failures trigger one independent stderr JSON diagnostic:
`record_type=audit_emit_failed`, ERROR level, generated ID, fixed event kind and safe category
(`invalid_payload`, `serialization`, `oversized`, `output`). It contains no rejected payload or
error text. Lines are strictly below 8 KiB; process-wide stream locks span write_all and flush.
There is no detached task, retry loop or CloudWatch SDK call. If both streams fail, no reliable
self-report is possible.

`none` invokes neither audit sink. It does not disable operational tracing or API Gateway access
logs. [Function logging](../../../infra/auth/logging.md) uses JSON/INFO and the existing groups/roles;
finite shared retention defaults to 30 days. Log ingestion/storage incur ordinary CloudWatch costs.

Local write success is not a CloudWatch delivery acknowledgement. Delivery is best effort, not
transactional, exactly once, tamper-proof or compliance-grade. Partial output can leave a broken
line. Multi-step admin cleanup can partially succeed without a completed-action event, and a
response can fail after an event is emitted. This does not sanitize unrelated operational logging.

No new DynamoDB audit rows are written. Historical rows without TTL remain outside CloudWatch
retention and are not read, migrated or deleted by this implementation. Any cleanup requires a
separate operator decision.
