# Admin Startup

Source: [packages/functions/admin/src/main.rs](../../../../packages/functions/admin/src/main.rs).

## What

The IAM-admin Lambda entrypoint.

## Why

Admin startup needs lifecycle storage and audit output, not provider/email/token-signing configuration.

## How

Creates a cached DynamoDB client and AuthStore, parses deletion policy and audit mode, configures
JSON tracing and runs create_admin_router. It explicitly installs the production stdout/stderr
AuditEmitter using the parsed mode. Mode loading defaults to cloudwatch, accepts none and rejects
unknown/empty values before serving requests; a unit test exercises the same loading function.
No Resend or provider client is initialized.
