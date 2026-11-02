# Design

This directory documents the current architecture and boundaries of the Irongate template.

The design tree mirrors the code the template owns:

```text
design/
  overview.md
  functions/
    admin/
      src/
    auth/
      src/
      tests/
  infra/
    auth/
    shared/
    examples/
  examples/
    web/
    app/
  scripts/
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
- `examples/app.md`: desktop-first Tauri app and its current platform boundaries.
- `examples/client-profiles.md`: OAuth client profile rules for web and native clients.

## Design Rule

Authoritative docs describe what exists, why it exists, and the security boundaries it preserves.
Follow `AGENTS.md` for exact source-to-design mappings, including `src/` and `tests/`, and add
file mirrors for touched implementation in the same slice. Existing topic documents remain
architecture overviews. New package, script and app mirrors use `design/packages/`,
`design/scripts/` and `design/apps/` as applicable.

Each file mirror links its source and states what it does, why it exists and how it works.
Configuration basenames such as `tauri.conf` and `tsconfig.test` are retained. When multiple
files map to one name, such as `App.tsx`/`App.css` or a script and its `.test.sh`, the mirror
explicitly covers both. Lockfiles, generated output, reference repositories and binary assets
are excluded. Implementation limits are documented rather than presented as completed guarantees.

`design/research/` contains non-authoritative investigations, explicitly requested handoffs and
operational notes. `design/implementation/` contains only the current and completed numbered
slices. Both are exempt from mirroring. Future plans stay in the conversation; authoritative
design must not accumulate superseded decisions or postponed proposals.

For the current cross-repository work, start with the
[Irongate authentication improvements handoff](research/irongate_auth_improvements_handoff.md).
