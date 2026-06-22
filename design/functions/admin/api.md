# Admin Function API

Target code: `packages/functions/admin/src/main.rs` plus the shared route module currently in
`packages/functions/auth/src/api/admin.rs`.

## Owns

- Operator-only account lifecycle routes.
- HTTP request and response formatting for admin actions.
- Mapping IAM-authenticated requests to core lifecycle operations.

## Target Boundary

The admin API is not a hosted operator UI and not a custom admin-key system.

Admin routes exist only for account lifecycle operations that are hard to operate safely through public auth flows:

```text
GET  /_admin/users/{subject}
POST /_admin/users/{subject}/disable
POST /_admin/users/{subject}/enable
POST /_admin/users/{subject}/delete
POST /_admin/users/{subject}/revoke-sessions
```

They do not create, update, rotate, disable, or delete OAuth clients. OAuth clients remain config-only in v1.

Admin routes should be served by a separate admin Lambda, not by the public auth Lambda's `$default` route. Shared account, identity, refresh-token, and audit logic should live in core/store modules so the public and admin entrypoints do not duplicate lifecycle rules.

## Authentication And Authorization

Admin routes must be protected by API Gateway IAM authorization:

```text
auth: { iam: true }
```

Operators call these routes with AWS Signature Version 4 using an IAM principal that has `execute-api:Invoke` for the specific admin route ARN. Unsigned or unauthorized requests should be rejected by API Gateway before Lambda invocation.

The admin Lambda should still treat admin routes as privileged and reject them if the expected API Gateway/IAM request context is missing.

## Security Invariants

- No public `/admin/bootstrap`.
- No custom standing admin API key.
- No browser or hosted-UI assumption.
- No CORS requirement for admin routes.
- Admin routes are not mounted behind the public `$default` Lambda integration.
- No raw token, password hash, verification link, reset link, provider state, or signing key is returned.
- Admin read responses are sanitized account status, not raw DynamoDB records.
- Every successful admin mutation emits an audit event.
- Admin operations use exact-key reads, bounded queries, or transactions; they do not scan the auth table.
