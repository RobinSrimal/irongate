# Example Infra

Target code: `infra/examples`

## Owns

Optional reference infrastructure for future examples:

- Hosted `auth-web`.
- Hosted `web-spa`.
- Hosted sample `resource-api`.
- Example app domains and hosting settings.
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

## Related Design

Example application architecture lives under `design/examples`.
