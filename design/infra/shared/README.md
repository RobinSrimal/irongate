# Shared Infra

Target code: `infra/shared`

## Owns

- Version-controlled non-secret stage config.
- Stage/account naming inputs.
- Rust Lambda bundling helper.
- Small reusable infra helpers that do not create resources at import time.

## Must Not Own

- API Gateway routes.
- Lambda resources.
- DynamoDB resources.
- KMS resources.
- SST secrets.
- Example hosting resources.

## Import Boundary

`infra/shared` can be imported by `sst.config.ts`, `infra/auth`, and `infra/examples`.

Shared modules must not create AWS resources at import time. They exist to keep config and helper code out of either side of the auth/example boundary.

## Design Files

- `stages.md`: dev/prod account, stage config, and example enablement model.
