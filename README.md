<p align="center">
  <img src="assets/irongate-logo.png" alt="Irongate logo" width="256" />
</p>

<p align="center">
  Rust and AWS auth you can inspect, deploy, and own.
</p>

Irongate is a Rust and AWS auth template for applications that want a small, inspectable auth
foundation instead of a hosted auth black box.

It gives you:

- A Rust OAuth/OIDC auth server deployed to AWS Lambda.
- Password auth with email verification and password reset.
- Google and Apple OIDC login.
- Self-contained JWT access tokens and OIDC ID tokens.
- Refresh-token rotation and logout.
- IAM-protected account lifecycle admin routes.
- DynamoDB storage with typed auth records and HMAC lookup keys for bearer-style secrets.
- SST infrastructure for API Gateway, Lambda, DynamoDB, secrets, logs, and optional KMS.
- Optional example clients: a Cloudflare Worker BFF web app and a Tauri desktop app.

## Architecture

Default core deployment:

```text
API Gateway HTTP API
  -> public Rust auth Lambda
  -> IAM-protected Rust admin Lambda
  -> DynamoDB AuthTable
  -> SST secrets, CloudWatch logs, optional KMS
```

Optional examples:

```text
Cloudflare Worker web BFF
  -> Irongate auth API
  -> Durable Object session storage

Tauri desktop app
  -> Irongate auth API
  -> OS keychain refresh-token storage
```

The auth core is API-only. Applications own their login, registration, reset, provider-selection,
and error screens. The included examples demonstrate secure integration patterns without becoming
part of the core deploy.

## Why Irongate

Irongate is meant to be understood and modified by the team that deploys it.

The template keeps security-sensitive choices explicit:

- OAuth clients are config-only and reviewed through repo/deploy changes.
- Runtime admin lifecycle routes are isolated in a separate IAM-protected Lambda.
- Password registration never authenticates before email verification.
- Refresh tokens, authorization codes, provider state, verification links, and reset links are stored
  by HMAC lookup digest.
- Resend is the email delivery path in deployed stages.
- Dev can use local ES256 signing to avoid KMS signing cost; production can use KMS signing.

## Start Here

Use the docs one step at a time:

```text
docs/setup/01-template-setup.md
docs/setup/02-aws-accounts-and-sst.md
docs/setup/03-stage-config.md
docs/setup/04-secrets.md
docs/setup/05-auth-clients.md
docs/setup/06-resend-email.md
docs/setup/09-deploy-auth-core.md
```

Provider and example setup:

```text
docs/setup/07-google-login.md
docs/setup/08-apple-login.md
docs/setup/10-cloudflare-account.md
docs/setup/11-cloudflare-web-example.md
docs/setup/12-tauri-app-example.md
```

Operations:

```text
docs/operations/smoke-test.md
docs/operations/local-signing-dev.md
```

## Use As A Template

Create a new repository from this template, clone it, then run the setup script to rename the project:

```bash
npm run setup
```

After that, fill in `infra/shared/stage-config.ts`, set the required SST secrets, review
`auth.clients.toml`, and deploy the auth core:

```bash
npm run deploy -- --stage dev
```

The full setup path lives in `docs/setup/01-template-setup.md`.

## Secret Boundary

The template deliberately separates local deploy credentials, runtime secrets, and reviewed
configuration:

- `.env`: local-only tooling credentials, such as Cloudflare API token and account ID.
- SST secrets: Irongate runtime secrets used by Lambda, such as Resend, HMAC lookup, signing, Google,
  Apple, and confidential client secrets.
- `infra/shared/stage-config.ts`: checked-in non-secret stage config.
- `auth.clients.toml`: checked-in OAuth client definitions and secret reference names.

See `docs/setup/04-secrets.md` and `docs/setup/10-cloudflare-account.md`.

## Repository Layout

```text
auth.clients.toml              OAuth client definitions
infra/                         SST infrastructure
packages/functions/auth/       public Rust auth Lambda
packages/functions/admin/      IAM-protected Rust admin Lambda
packages/examples/web/         optional Cloudflare Worker BFF example
packages/examples/app/         optional Tauri desktop app example
docs/                          setup and operation guides
design/                        architecture and boundary notes
```

## Design

Start with:

```text
design/overview.md
design/functions/README.md
design/infra/README.md
design/examples/README.md
```
