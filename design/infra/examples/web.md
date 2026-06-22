# Web Example Infra

Target code: `infra/examples/web.ts`

## Owns

- Optional Cloudflare Worker deployment for `packages/examples/web`.
- Cloudflare Worker URL output.
- Optional Cloudflare custom domain configuration.
- Durable Object migration for web BFF session state.
- Worker environment for Irongate issuer/client integration.

## Must Not Own

- Irongate auth core AWS resources.
- Irongate DynamoDB auth table.
- Runtime auth/admin Lambda configuration.
- Cloudflare KV for auth/session state.
- Native app infrastructure.
- Shared resource API infrastructure.

## Deployment Boundary

The web example is enabled for the repo's dev stage so the deployed BFF can be smoke-tested, and
disabled for production:

```text
dev.examples.enabled = true
dev.examples.web.enabled = true
production.examples.enabled = false
production.examples.web.enabled = false
```

When enabled, it deploys a Cloudflare Worker BFF:

```text
Cloudflare Worker
  -> Durable Object session storage
  -> Irongate auth API on AWS
```

The default auth-core deploy must not create this Worker or any Cloudflare resources.

## Cloudflare Credentials

Deploying the web example requires Cloudflare credentials in the local shell environment:

```text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_DEFAULT_ACCOUNT_ID
```

The repo includes a committed `.example.env` file for these non-repo-local environment variable
names. Developers can copy it to `.env`, fill in their local values, and load it before deploying.
The `.env` file is git-ignored and must not be committed.

AWS remains handled by the stage's configured SSO profile. Irongate runtime secrets such as Resend,
HMAC lookup secret, and local signing private keys remain SST secrets, not `.env` values.

## Worker URL And Domains

For first dev deploys, the web example may use its generated `workers.dev` URL. The Worker derives
its public origin from incoming requests, so its deployed URL does not need to be injected into
`WEB_BASE_URL` before the Worker starts.

Production examples should configure an explicit Cloudflare domain:

```text
examples.web.domain = "auth-demo.example.com"
```

Irongate still requires exact redirect URI registration:

```text
https://auth-demo.example.com/auth/callback
```

`examples.web.baseUrl` remains an optional override for custom domains, tunnels, or unusual proxy
setups. It should not be required for the generated `workers.dev` first deploy.

## Durable Objects

Web session state belongs in Durable Objects:

```text
SESSION_OBJECT -> WebSessionObject
```

The Durable Object stores authoritative BFF session state and server-held refresh-token state.
Cloudflare KV is not used for:

- sessions
- refresh tokens
- OAuth state
- CSRF state
- logout state

## Worker Environment

Required environment:

```text
IRONGATE_ISSUER_URL
IRONGATE_CLIENT_ID
IRONGATE_GOOGLE_LOGIN_ENABLED
IRONGATE_APPLE_LOGIN_ENABLED
```

Optional environment:

```text
WEB_BASE_URL
```

`IRONGATE_GOOGLE_LOGIN_ENABLED` is a non-secret boolean derived from stage config. It only controls
whether the optional web example renders a Google login action. The Google client ID and secret are
used by the Irongate auth Lambda, not by browser JavaScript.

`IRONGATE_APPLE_LOGIN_ENABLED` is a non-secret boolean derived from stage config. It only controls
whether the optional web example renders an Apple login action. Apple identifiers and private-key
material are used by the Irongate auth Lambda, not by browser JavaScript.

The Worker must not receive Irongate HMAC secrets, Resend secrets, signing keys, AWS credentials, or
raw DynamoDB access.

## Production Hardening

Before treating the web example as production-ready, add a Worker-side allowed-origin guard:

```text
request origin must be in configured allowed origins
otherwise fail before calling Irongate
```

Irongate exact redirect URI matching is still the primary OAuth protection, but failing early in the
Worker reduces noise and makes misrouted traffic easier to understand.
