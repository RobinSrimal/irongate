# Design

This directory describes the target shape of the repository before code is moved.

The rule is symmetry: the design tree should match the code tree we intend to create. Each folder
explains what the corresponding code folder owns, why the boundary exists, how it works, and the
security invariants it preserves.

The current implementation still contains legacy/general-purpose pieces. These docs describe the narrower AWS-first auth template we want to refactor toward.

## Target Shape

```text
design/
  functions/
    admin/
    auth/
  examples/
  infra/
```

Target code symmetry:

```text
packages/functions/admin/src/
  main.rs

packages/functions/auth/src/
  api/
  core/
  store/
  crypto/
  providers/
  email/
  config/
  observability/

infra/
  auth/
  shared/
  examples/
sst.config.ts
```

Infra stays small because SST owns most AWS wiring. Function docs are more detailed because the
Rust Lambdas carry the security model.

## Existing Notes

The root-level docs are source material for this design tree:

- `AUTH_FLOWS.md`
- `AWS_INFRASTRUCTURE_OPTIMIZATION.md`
- `STORAGE_SECURITY.md`
- `SECURITY_SCAN.md`

They can be split or moved after the design tree settles.

## Cross-Cutting Docs

- `overview.md`: high-level template shape, boundaries, and token model.
- `functions/`: Rust Lambda function design for public auth and IAM admin entrypoints.
- `examples/`: optional example application architecture.
- `infra/auth/aws-dev-smoke-test.md`: recorded AWS dev deployment validation.
