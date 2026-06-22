# Admin Function Design

Target code: `packages/functions/admin`

## Owns

- IAM-protected account lifecycle Lambda entrypoint.
- Runtime wiring for admin-only configuration.
- Calling the shared admin router and lifecycle store operations.

## Boundaries

- Public OAuth, OIDC, password, Google, or Apple routes.
- Client-facing email delivery.
- Provider secrets.
- JWT signing private keys.
- Runtime OAuth client management.

## Runtime Boundary

The admin function is deployed separately from the public auth function and is reached only through
explicit `/_admin/*` API Gateway routes with IAM authorization enabled.

It should receive only the environment variables and permissions needed for account lifecycle
operations:

- DynamoDB table name.
- Audit logging mode.
- Deleted identity reuse and retention settings.
- KMS permissions only where required by DynamoDB.

The function relies on API Gateway IAM authorization and SigV4-signed operator requests.
