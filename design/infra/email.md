# Infra Email

Target code: `infra/api.ts`, SST secrets/config, and Resend account setup docs.

## Owns

- Runtime email configuration passed to the auth Lambda.
- Required Resend secrets.
- Sender address expectations.

## Decision

Resend is required in dev and production.

There is no runtime `EMAIL_PROVIDER` switch, no console email provider, and no SMTP/SES provider in the target core.

## Required Config

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
```

Optional:

```text
AUTH_EMAIL_REPLY_TO
AUTH_EMAIL_BRAND_NAME
AUTH_EMAIL_SUPPORT_EMAIL
AUTH_EMAIL_VERIFY_SUBJECT
AUTH_EMAIL_RESET_SUBJECT
AUTH_EMAIL_VERIFY_TEMPLATE_PATH
AUTH_EMAIL_RESET_TEMPLATE_PATH
```

## Resend Domain Setup

Sender/domain verification is handled in Resend and DNS.

The template user must:

- Create a Resend account.
- Add and verify the sending domain or sender allowed by Resend.
- Configure required DNS records in their DNS provider.
- Set `AUTH_EMAIL_FROM` to a sender allowed by the Resend account.
- Store `RESEND_API_KEY` as a secret for each stage/account.
- Package any configured email template override files with the auth Lambda artifact.

## Stage Strategy

Dev and prod use the same config shape but different credentials/senders:

```text
dev:  login@dev.example.com or allowed dev sender
prod: login@example.com
```

## Security Invariants

- `RESEND_API_KEY` is never logged.
- Email delivery success does not verify a user.
- Only consuming the verification secret marks a user verified.
- Non-auth tooling does not need Resend access.
- Email template paths are deploy-time config and must not be derived from request input.
