# Example Infrastructure

This folder contains optional example deployments.

The default Irongate deploy does not create frontend hosting, sample apps, or
sample application resources unless a stage enables them. The current dev stage
enables the web example for smoke testing; production keeps examples disabled.

Current optional resources:

- `ExampleWebWorker`: Cloudflare Worker BFF for the web password-auth example.

The web example uses Cloudflare Durable Objects for authoritative session state.
Cloudflare KV is not used for auth/session state.

The Worker derives its own base URL from incoming requests. `examples.web.baseUrl`
is optional and only sets `WEB_BASE_URL` when a stage needs to override the request
origin, for example behind a custom domain or tunnel.

## Local Environment

Cloudflare deploy credentials should be set in the local shell environment. Copy
the committed example file:

```bash
cp .example.env .env
```

Then fill in:

```text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_DEFAULT_ACCOUNT_ID
```

The `.env` file is git-ignored. The repo's `deploy` and `remove` npm scripts load it
automatically when it exists:

```bash
npm run deploy -- --stage dev
```
