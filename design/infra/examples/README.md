# Example Infra

Target code: `infra/examples`

## Owns

Optional reference infrastructure for example apps:

- Web example infrastructure under `infra/examples/web`.
- App example infrastructure under `infra/examples/app`.
- Example outputs when examples are explicitly enabled.

## Must Not Own

- Irongate auth core resources.
- DynamoDB auth table.
- Auth/admin Lambda routes.
- Auth secrets needed only by the core.
- Production core defaults.

## Import Boundary

`infra/examples` is disabled by default and must not be imported unless a stage enables examples deliberately:

```text
examples.enabled = true
```

SST creates resources at module import time, so this import gate is part of the product boundary. Example resources must not appear in the default auth-core deploy.

## Example Boundaries

Example infrastructure should not become a shared application platform inside this repository.
Each example owns its own deployable shape:

```text
web -> Cloudflare Worker BFF + Durable Object sessions
app -> native app support outputs only, until a concrete app deploy need exists
```

The web example may expose protected routes later, but those routes still belong to the web example.
Do not introduce a separate shared resource API package or shared example infrastructure until a
future design explicitly adds it.

## Related Design

Example application architecture lives under `design/examples`.

## Design Files

- `web.md`: Cloudflare web example infrastructure.
- `app.md`: native app example infrastructure.
