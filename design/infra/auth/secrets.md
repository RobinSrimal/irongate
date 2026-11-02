# Auth Secrets

Source: [infra/auth/secrets.ts](../../../infra/auth/secrets.ts).

## What

SST secret declarations for the HMAC lookup secret, Resend API key, Google client secret and Apple
private key. Local signing's secret is declared separately in [signing](signing.md).

## Why

Runtime credentials must not be committed with non-secret client/stage configuration or mixed with
local cloud deployment credentials.

## How

| SST secret | Public Lambda environment | Required when |
| --- | --- | --- |
| AuthHmacLookupSecret | AUTH_HMAC_LOOKUP_SECRET | Always |
| ResendApiKey | RESEND_API_KEY | Always |
| GoogleClientSecret | AUTH_GOOGLE_CLIENT_SECRET | Google client ID configured |
| ApplePrivateKey | AUTH_APPLE_PRIVATE_KEY | Apple enabled |
| AuthSigningPrivateKey | AUTH_SIGNING_PRIVATE_KEY | local-es256 selected |

Values are set per stage/account and bound in `api.ts`. Non-secret provider IDs, issuer, audience,
callback URLs, sender and brand belong in `infra/shared/stage-config.ts`. KMS key identifiers are
resource references, not secret values. Local private keys reach Lambda through SST secret-backed
environment values, not AuthTable.

Confidential clients can name a `client_secret_ref`; operators must explicitly declare and bind that
secret into the public Lambda environment. The current infrastructure does not automatically create
bindings for arbitrary references found in `auth.clients.toml`.

Local `.env` holds optional Cloudflare deployment credentials and AWS profile overrides. Runtime
provider/HMAC/signing secrets stay in SST secrets. The admin Lambda receives none of these public
runtime secrets. See [secret setup](../../../docs/setup/04-secrets.md) for commands.

Changing the HMAC lookup secret changes lookup addresses; the current runtime has no multi-key
lookup migration. Secret values and private-key files must not enter git or logs.
