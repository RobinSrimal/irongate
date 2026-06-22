# Auth Infra

Target code: `infra/auth`

## Owns

- API Gateway HTTP API.
- Public Rust auth Lambda.
- Separate Rust admin Lambda for IAM-protected account lifecycle routes.
- DynamoDB auth table.
- Optional customer managed DynamoDB table KMS key.
- Optional KMS signing key.
- Auth runtime environment variables and SST secrets.
- Runtime IAM permissions for auth/admin Lambdas.

## Boundaries

- Frontend hosting and example applications live under `infra/examples`.
- Shared stage config helpers live under `infra/shared`.
- Business application functions live outside the auth/admin Lambda boundary.

## Import Boundary

`infra/auth` is imported by default and creates the core Irongate AWS resources. Files in this folder may create SST/AWS resources at import time.

`infra/auth` may import from `infra/shared`. Example resources are imported only through the
top-level opt-in example gate.

## Design Files

- `api.md`: API Gateway decisions.
- `auth-function.md`: Rust Lambda deployment shape.
- `storage.md`: DynamoDB and KMS decisions.
- `secrets.md`: provider credentials and auth secrets.
- `email.md`: Resend setup and sender configuration.
- `iam.md`: runtime and operator IAM boundaries.
- `operator-iam-policy.md`: operator policy examples for admin routes.
- `performance.md`: Lambda sizing, client reuse, timeout, and load-test guidance.
