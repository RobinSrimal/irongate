<p align="center">
  <img src="assets/irongate-logo.png" alt="Irongate logo" width="256" />
</p>

Irongate gives you serverless Auth on AWS: scalable, reliable, performant and secure. Built with [SST](https://sst.dev) you can use this template as a starting point to build your own web or app projects while keeping full control over the auth layer. 

## Why Irongate

I never understood why we are willing to pay the high MAU costs for services such as Cognito. Irongate uses powerful primitives by AWS to ensure scalability and reliability. Rust based Lambdas keep it performant and the code itself is yours to audit if you have security concerns.  

Not to mention that the AWS Free Tier will go a long way if you are just ramping up your first users. 

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
