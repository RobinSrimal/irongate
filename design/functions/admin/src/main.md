# Admin Startup

Source: [packages/functions/admin/src/main.rs](../../../../packages/functions/admin/src/main.rs).

## What

The IAM-admin Lambda entrypoint.

## Why

Admin startup needs lifecycle storage, not provider/email/token-signing configuration.

## How

Creates a cached DynamoDB client and AuthStore, parses deletion policy and audit mode, configures JSON tracing and runs create_admin_router. No Resend or provider client is initialized.
