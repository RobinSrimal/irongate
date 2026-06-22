# Cloudflare Account

## Goal

Configure local Cloudflare credentials for optional example deployments.

## Inputs Needed

- Cloudflare account with Workers access.
- Cloudflare account ID.
- Cloudflare API token with permissions for Worker deploys.

## Files To Edit

- Copy `.example.env` to `.env`.
- Do not edit SST secrets for Cloudflare credentials.

## Secret Boundary

Cloudflare deploy credentials go in local `.env` because they are used by SST/Pulumi from the local
machine during example deployment:

```text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_DEFAULT_ACCOUNT_ID
```

Irongate runtime secrets stay in SST secrets because they are used by the AWS Lambda runtime:

```text
AuthHmacLookupSecret
ResendApiKey
AuthSigningPrivateKey
GoogleClientSecret
ApplePrivateKey
OAuth confidential client secrets
```

Non-secret provider IDs stay in checked-in stage config:

```text
infra/shared/stage-config.ts
```

## Commands

```bash
cp .example.env .env
```

Fill in:

```text
CLOUDFLARE_API_TOKEN=<token>
CLOUDFLARE_DEFAULT_ACCOUNT_ID=<account id>
```

The root deploy script loads `.env` automatically:

```bash
npm run deploy -- --stage dev
```

## Validation

Check that `.env` is ignored:

```bash
git status --ignored .env
```

Expected:

```text
!! .env
```

Check that the variables are available to a local shell:

```bash
set -a
source .env
set +a
test -n "$CLOUDFLARE_API_TOKEN"
test -n "$CLOUDFLARE_DEFAULT_ACCOUNT_ID"
```

## Common Failures

- Putting Cloudflare credentials in SST secrets; SST needs them locally to create Cloudflare
  resources.
- Putting Irongate runtime secrets in `.env`; deployed AWS Lambdas need SST secrets.
- Forgetting to enable examples in `infra/shared/stage-config.ts`.
- Using a token that can read the account but cannot deploy Workers.

## Done When

- `.env` contains Cloudflare deploy credentials.
- `.env` is ignored by git.
- Irongate runtime secrets remain in SST secrets.
