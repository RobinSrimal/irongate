# Shared Infra

Target code: `infra/shared`

## Owns

- Version-controlled non-secret stage config.
- Stage/account naming inputs.
- Rust Lambda bundling helper.
- Small reusable infra helpers that do not create resources at import time.

## Boundaries

- API Gateway, Lambda, DynamoDB, KMS, and SST secret resources are created by resource-owning infra
  modules.
- Example hosting resources are created by `infra/examples`.

## Import Boundary

`infra/shared` can be imported by `sst.config.ts`, `infra/auth`, and individual example infra
modules.

Shared modules are import-safe: they provide config and helper code without creating AWS,
Cloudflare, or other third-party resources at import time.

## Design Files

- `stages.md`: dev/prod account, stage config, and example enablement model.
