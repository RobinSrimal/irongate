# Design

This directory documents the current architecture and boundaries of the Irongate template.

The design tree mirrors the code the template owns:

```text
design/
  overview.md
  functions/
    admin/
    auth/
  infra/
    auth/
    shared/
    examples/
  examples/
```

## What To Read First

- `overview.md`: cross-cutting template scope, function boundaries, examples, and token model.
- `functions/README.md`: Rust Lambda boundaries.
- `infra/README.md`: SST infrastructure boundaries.
- `examples/README.md`: optional web and app example boundaries.

## Function Design

Function docs mirror `packages/functions`.

```text
packages/functions/
  auth/
  admin/
```

- `functions/auth/`: public auth Lambda, OAuth/OIDC flows, password auth, providers, typed store,
  crypto, email, config, observability, testing, and threat model.
- `functions/admin/`: IAM-protected account lifecycle Lambda.

The admin function may reuse shared auth modules, but it has its own deployed entrypoint, route
surface, runtime environment, and IAM boundary.

## Infra Design

Infra docs mirror `infra`.

```text
infra/
  auth/
  shared/
  examples/
```

- `infra/auth/`: AWS API Gateway, Lambda, DynamoDB, secrets, logging, IAM, KMS, and email delivery.
- `infra/shared/`: stage config and helper modules that should not create resources at import time.
- `infra/examples/`: optional example deployment resources.

The default deploy creates only the auth core. Example infrastructure is imported only when a stage
explicitly enables examples.

## Example Design

Example docs describe optional reference implementations, not core requirements.

- `examples/web.md`: Cloudflare Worker BFF web app.
- `examples/app.md`: desktop-first Tauri app with mobile notes.
- `examples/client-profiles.md`: OAuth client profile rules for web and native clients.

## Design Rule

These docs should describe what exists, why it exists, and the security boundaries it preserves.
They should not carry historical migration plans or postponed product ideas.
