# Infra Design

Infra should remain thin. SST is the infrastructure abstraction, so this folder documents only the deployment decisions the template owns.

Target code:

```text
infra/
  auth/
    api.ts
    config.ts
    secrets.ts
    signing.ts
    storage.ts
  shared/
    rust-bundle.ts
    stage-config.ts
  examples/
    README.md
    config.ts
    index.ts
sst.config.ts
```

## Owns

- API Gateway HTTP API.
- Public Rust auth Lambda.
- Separate Rust admin Lambda for IAM-protected account lifecycle routes.
- DynamoDB auth table.
- Optional customer managed KMS for production.
- Stage/account naming.
- Runtime environment variables and secrets.

## Must Not Own

- Auth protocol logic.
- Provider-specific login logic.
- Business application functions as part of the core deploy.
- Frontend hosting or reference applications unless explicitly enabled as examples.

## Examples Boundary

Frontend hosting and reference applications live under `infra/examples` and are disabled by default. The default deploy imports and creates only `infra/auth` resources. Example modules must remain opt-in because SST creates resources at module import time.

## Design Files

- `api.md`: API Gateway decisions.
- `auth-function.md`: Rust Lambda deployment shape.
- `storage.md`: DynamoDB and KMS decisions.
- `secrets.md`: provider credentials and auth secrets.
- `stages.md`: dev/prod account and naming model.
- `email.md`: Resend setup and sender configuration.
- `iam.md`: runtime and operator IAM boundaries.
- `performance.md`: Lambda sizing, client reuse, timeout, and load-test guidance.
