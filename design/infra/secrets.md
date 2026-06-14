# Infra Secrets

Target code: `sst.config.ts`, `infra/api.ts`, and SST secret bindings.

## Owns

- How provider credentials are supplied.
- How auth-internal secrets are supplied.
- Stage-specific secret names.

## Required Secret Families

- HMAC lookup secret for token/code lookup digests.
- `RESEND_API_KEY` for password verification and reset email delivery.
- Google client secret when Google login is enabled.
- Apple private key or client-secret inputs when Apple login is enabled.
- JWT signing KMS key ID or local signing-key decryption secret, depending on signing mode.
- Optional customer managed KMS key configuration for the DynamoDB table.

## Target Behavior

Local development may read environment variables directly. Deployed stages should use SST secrets or AWS-native secret storage rather than hardcoded values.

Startup should fail clearly when required secrets are missing. Resend is required for both dev and production because password auth depends on verification and reset email.

## Security Invariants

- Secrets must not be written into DynamoDB records.
- Secrets must not appear in logs.
- The HMAC lookup secret should be rotated through a documented migration path, not by silently invalidating every token.
- The HMAC lookup secret must be distinct from JWT signing keys and provider credentials.
- Signing private key material should not be available to generic AuthTable readers.
- Resend secrets are runtime-only and must not be exposed to non-auth tooling.
