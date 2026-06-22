# Irongate Docs

These docs are operational guides for setting up and running the template.

Use them one at a time. Each guide is written so a developer or coding agent can gather the required
inputs, edit the right files, run the commands, and validate the result without inferring the whole
system.

## Setup

- `setup/01-template-setup.md`: create a repository from the template and rename it.
- `setup/02-aws-accounts-and-sst.md`: configure AWS SSO profiles and SST stages.
- `setup/03-stage-config.md`: edit checked-in non-secret stage configuration.
- `setup/04-secrets.md`: set Irongate runtime secrets with SST.
- `setup/05-auth-clients.md`: configure OAuth clients in `auth.clients.toml`.
- `setup/06-resend-email.md`: configure verification and reset email delivery.
- `setup/07-google-login.md`: configure Google login.
- `setup/08-apple-login.md`: configure Sign in with Apple.
- `setup/09-deploy-auth-core.md`: deploy and validate the AWS auth core.
- `setup/10-cloudflare-account.md`: configure Cloudflare credentials for optional examples.
- `setup/11-cloudflare-web-example.md`: deploy and validate the Cloudflare web BFF example.
- `setup/12-tauri-app-example.md`: run the desktop app example.

## Operations

- `operations/admin-lifecycle.md`: call IAM-protected account lifecycle routes.
- `operations/local-signing-dev.md`: use local ES256 signing for dev.
- `operations/smoke-test.md`: run basic deployed smoke checks.

## Secret Boundary

- `.env` is only for local shell variables needed by optional local/example tooling, especially
  Cloudflare deploy credentials.
- SST secrets hold Irongate runtime secrets used by Lambda.
- `infra/shared/stage-config.ts` holds reviewed, non-secret stage configuration.
- `auth.clients.toml` holds non-secret OAuth client definitions and secret reference names.
