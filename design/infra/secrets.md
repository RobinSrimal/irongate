# Infra Secrets

Target code: `sst.config.ts`, `infra/api.ts`, and SST secret bindings.

## Owns

- How provider credentials are supplied.
- How auth-internal secrets are supplied.
- Stage-specific secret names.

## Required Secret Families

- HMAC lookup secret for token/code lookup digests.
- `RESEND_API_KEY` for password verification and reset email delivery.
- Confidential OAuth client secrets referenced by `auth.clients.toml`.
- Google client secret when Google login is enabled.
- Apple private key or client-secret inputs when Apple login is enabled.
- JWT signing KMS key ID or local signing-key decryption secret, depending on signing mode.
- Local ES256 signing private key secret when `AUTH_SIGNING_MODE=local-es256`.
- KMS asymmetric signing key when `AUTH_SIGNING_MODE=kms-es256`.
- Optional customer managed KMS key configuration for the DynamoDB table.

## Target Behavior

Local development may read environment variables directly. Deployed stages should use SST secrets for application secrets rather than hardcoded values.

V1 stores non-secret OAuth client definitions in `auth.clients.toml`. That file may contain `client_secret_ref` names, but it must not contain raw client secrets. Each `client_secret_ref` must resolve to an SST secret in deployed stages.

Startup should fail clearly when required secrets are missing. Resend is required for both dev and production because password auth depends on verification and reset email.

## SST Secret Boundary

Use SST secrets for values such as:

```text
AUTH_HMAC_LOOKUP_SECRET
RESEND_API_KEY
AUTH_CLIENT_BACKEND_SECRET
AUTH_SIGNING_PRIVATE_KEY
AUTH_GOOGLE_CLIENT_SECRET
APPLE_PRIVATE_KEY
```

Do not use SST secrets to hide non-secret configuration like redirect URIs, allowed scopes, grant types, or client type. Those belong in checked-in config so they can be reviewed.

## Security Invariants

- Secrets must not be written into DynamoDB records.
- Secrets must not appear in logs.
- Raw secrets must not appear in `auth.clients.toml`.
- The HMAC lookup secret should be rotated through a documented migration path, not by silently invalidating every token.
- The HMAC lookup secret must be distinct from JWT signing keys and provider credentials.
- Signing private key material should not be available to generic AuthTable readers.
- Resend secrets are runtime-only and must not be exposed to non-auth tooling.
