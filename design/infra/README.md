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
    web.ts
    app.ts
    config.ts
    index.ts
sst.config.ts
```

## Infra Boundaries

The infra design tree mirrors the code tree:

- `auth/`: Irongate core AWS resources.
- `shared/`: config and helper modules that create no resources at import time.
- `examples/`: optional example deployment resources, disabled by default.

## Boundaries

- Auth protocol and provider-specific login logic live in `packages/functions/auth`.
- Business application functions live outside the core deploy.
- Frontend hosting and reference applications live under opt-in examples.

## Import Boundary

SST creates resources at module import time, so imports are part of the security and product boundary.

Default deploy:

```text
sst.config.ts
  -> infra/shared/stage-config
  -> infra/auth/storage
  -> infra/auth/signing
  -> infra/auth/api
```

Opt-in example deploy:

```text
if examples.enabled:
  -> infra/examples
```

The example architecture is documented under `design/examples`. Example infrastructure is split by
example app:

- `examples/web`: Cloudflare Worker BFF and Durable Object session storage.
- `examples/app`: native app support outputs.

Example infrastructure is outside the default auth-core deploy.

## Design Files

- `auth/README.md`: core auth infrastructure boundary.
- `shared/README.md`: shared infra helper boundary.
- `examples/README.md`: optional example infra boundary.
- `examples/web.md`: web example Cloudflare infrastructure.
- `examples/app.md`: app example infrastructure boundary.
