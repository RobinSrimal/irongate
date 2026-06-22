# SST Secrets

## Goal

Set Irongate runtime secrets for each SST stage.

## Inputs Needed

- HMAC lookup secret.
- Resend API key.
- Local ES256 private key if the stage uses `local-es256`.
- Google client secret if Google login is enabled.
- Apple private key if Apple login is enabled.
- Confidential OAuth client secrets if configured in `auth.clients.toml`.

## Files To Edit

None for secret values.

Secret reference names may appear in:

- `auth.clients.toml`
- `infra/shared/stage-config.ts`

## SST Secrets

Required for auth runtime:

```bash
npx sst secret set AuthHmacLookupSecret "<32+ byte random secret>" --stage dev
npx sst secret set ResendApiKey "<resend api key>" --stage dev
```

Required when dev uses local ES256 signing:

```bash
npx sst secret set AuthSigningPrivateKey --stage dev < signing-dev.pem
```

Required when Google login is enabled:

```bash
npx sst secret set GoogleClientSecret "<google oauth client secret>" --stage dev
```

Required when Apple login is enabled:

```bash
npx sst secret set ApplePrivateKey --stage dev < AuthKey_<KEY_ID>.p8
```

Repeat secrets for production with production values:

```bash
npx sst secret set AuthHmacLookupSecret "<prod secret>" --stage production
npx sst secret set ResendApiKey "<prod resend key>" --stage production
```

## `.env` Boundary

`.env` is for local shell variables used by optional tooling, especially Cloudflare example deploys.

Keep these in `.env`:

```text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_DEFAULT_ACCOUNT_ID
SST_DEV_AWS_PROFILE optional override
SST_PROD_AWS_PROFILE optional override
```

Keep Irongate runtime secrets in SST secrets:

```text
AuthHmacLookupSecret
ResendApiKey
AuthSigningPrivateKey
GoogleClientSecret
ApplePrivateKey
OAuth confidential client secrets
```

## Validation

Run a deploy. SST fails clearly when a required secret is missing:

```bash
npm run deploy -- --stage dev
```

## Common Failures

- Putting Lambda runtime secrets in `.env`; deployed Lambdas cannot read local `.env`.
- Forgetting `--stage dev` or `--stage production` when setting a secret.
- Setting Apple private key without PEM headers.
- Keeping `signing-dev.pem` inside the repo without `.gitignore`; the template ignores
  `signing-*.pem`.

## Done When

- Required stage secrets are set for the stage being deployed.
- No runtime secret values are committed to the repo.
