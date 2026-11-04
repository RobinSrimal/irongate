# Public Lambda Startup

Source: [packages/functions/auth/src/main.rs](../../../../packages/functions/auth/src/main.rs).

## What

The bootstrap entrypoint for the public auth Lambda.

## Why

AWS clients and provider caches are reused within warm execution environments.

## How

Initializes JSON tracing, cached AWS config/DynamoDB client, validated runtime configuration, Resend and Google/Apple clients. AuthStore wraps DynamoStorage. AppState explicitly installs the production stdout/stderr AuditEmitter
from the validated audit mode; lambda_http runs the public Axum router. No admin router is mounted here.
