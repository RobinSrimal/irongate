# Example Infra

Target code: `infra/examples`

## Owns

Optional reference infrastructure for example apps:

- Web example infrastructure under `infra/examples/web`.
- App example infrastructure under `infra/examples/app`.
- Example outputs when examples are explicitly enabled.

## Boundaries

- Irongate auth core resources live under `infra/auth`.
- Auth/admin Lambda routes live under `infra/auth/api.ts`.
- Core-only secrets stay bound to the auth/admin Lambdas.
- Production core defaults live in shared stage config.

## Import Boundary

`infra/examples` is imported only when a stage enables examples deliberately:

```text
examples.enabled = true
```

SST creates resources at module import time, so this import gate is part of the product boundary.
Example resources are outside the default auth-core deploy.

## Example Boundaries

Each example owns its own deployable shape:

```text
web -> Cloudflare Worker BFF + Durable Object sessions
app -> native app support outputs
```

In this template, examples stay focused on auth integration.

## Related Design

Example application architecture lives under `design/examples`.

## Design Files

- `web.md`: Cloudflare web example infrastructure.
- `app.md`: native app example infrastructure.
