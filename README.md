<p align="center">
  <img src="assets/irongate-logo.png" alt="Irongate logo" width="256" />
</p>

Irongate is a serverless authentication template for AWS. It gives you a Rust-based auth core running on API Gateway, Lambda, and DynamoDB, deployed with [SST](https://sst.dev).
It is meant for teams that want to own their auth layer without running servers or paying hosted-auth MAU pricing. You get a small, inspectable starting point that you can deploy, modify, and extend inside your own AWS account.

## Getting Started

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

## Configuration

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

## Secret Boundary

The template deliberately separates local deploy credentials, runtime secrets, and reviewed
configuration:

- `.env`: local-only tooling credentials, such as Cloudflare API token and account ID.
- SST secrets: Irongate runtime secrets used by Lambda, such as Resend, HMAC lookup, signing, Google,
  Apple, and confidential client secrets.
- `infra/shared/stage-config.ts`: checked-in non-secret stage config.
- `auth.clients.toml`: checked-in OAuth client definitions and secret reference names.

See `docs/setup/04-secrets.md` and `docs/setup/10-cloudflare-account.md`.

## Design

Start with:

```text
design/overview.md
design/functions/README.md
design/infra/README.md
design/examples/README.md
```
