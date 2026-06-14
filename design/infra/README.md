# Infra Design

Infra should remain thin. SST is the infrastructure abstraction, so this folder documents only the deployment decisions the template owns.

Target code:

```text
infra/
  api.ts
  storage.ts
sst.config.ts
```

## Owns

- API Gateway HTTP API.
- One Rust auth Lambda.
- DynamoDB auth table.
- Optional customer managed KMS for production.
- Stage/account naming.
- Runtime environment variables and secrets.

## Must Not Own

- Auth protocol logic.
- Provider-specific login logic.
- Business application functions beyond wiring them into the SST app.

## Design Files

- `api.md`: API Gateway decisions.
- `auth-function.md`: Rust Lambda deployment shape.
- `storage.md`: DynamoDB and KMS decisions.
- `secrets.md`: provider credentials and auth secrets.
- `stages.md`: dev/prod account and naming model.
- `email.md`: Resend setup and sender configuration.
- `iam.md`: runtime and operator IAM boundaries.
- `performance.md`: Lambda sizing, client reuse, timeout, and load-test guidance.
