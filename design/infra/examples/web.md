# Web Example Infrastructure

Implementation: [infra/examples/index.ts](../../../infra/examples/index.ts);
file mirror: [index](index.md).

## What And Why

The optional Cloudflare Worker hosts the web BFF and binds a SQLite-backed Durable Object namespace.
It creates no extra AWS auth function or application table. Keeping it behind stage enablement
lets a fresh template deploy the auth core independently.

Both checked-in stages set `examples.enabled` and `examples.web.enabled` to false.

## How

When explicitly enabled, SST bundles `packages/examples/web/src/worker.ts`, binds
`SESSION_OBJECT` to `WebSessionObject`, and applies the Durable Object migration.
The Worker receives the generated AWS API URL, web client ID, provider-button flags and optional
`WEB_BASE_URL`. This wiring uses `api.url`, not an independently configured auth issuer override.

The generated `workers.dev` URL works as a first-deploy origin. A configured
`examples.web.domain` adds a custom domain; `baseUrl` alone does not provision one.
The actual callback still needs exact registration in `auth.clients.toml`.

## Credentials And Boundaries

Cloudflare deploy credentials `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_DEFAULT_ACCOUNT_ID` belong
in the local ignored `.env`, loaded by the repository's SST wrapper. AWS deployment uses the
stage's AWS profile. Provider, email, HMAC and signing secrets remain SST runtime secrets.

The Worker receives no AWS credentials or DynamoDB permissions. Durable Objects store session
records; their presence does not add workflow serialization, expiry enforcement or token refresh
to the Worker. See [current web behavior](../../examples/web.md) for those limits.
