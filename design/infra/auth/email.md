# Infra Email

Implementation: `infra/auth/api.ts`, SST secrets/config, and Resend account setup docs.

## Owns

- Runtime email configuration passed to the auth Lambda.
- Required Resend secrets.
- Sender address expectations.

## Delivery

Resend is required in dev and production.

There is no runtime `EMAIL_PROVIDER` switch, no console email provider, and no SMTP/SES provider in the target core.

## Required Config

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
AUTH_EMAIL_VERIFY_URL_BASE
AUTH_EMAIL_RESET_URL_BASE
```

Optional:

```text
AUTH_EMAIL_REPLY_TO
AUTH_EMAIL_BRAND_NAME
AUTH_EMAIL_SUPPORT_EMAIL
AUTH_EMAIL_VERIFY_SUBJECT
AUTH_EMAIL_RESET_SUBJECT
```

`AUTH_EMAIL_VERIFY_TEMPLATE_PATH` and `AUTH_EMAIL_RESET_TEMPLATE_PATH` are parsed by runtime
configuration but are not loaded by the current built-in renderer. They do not enable overrides.

## Resend Domain Setup

Sender/domain verification is handled in Resend and DNS.

The template user must:

- Create a Resend account.
- Add and verify the sending domain or sender allowed by Resend.
- Configure required DNS records in their DNS provider.
- Set `AUTH_EMAIL_FROM` to a sender allowed by the Resend account.
- Store `RESEND_API_KEY` as a secret for each stage/account.

Email config belongs to the auth core. The web example consumes verification links but receives
neither Resend credentials nor mail-delivery permissions. Reset requires an app-owned handler.

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
- Email rendering uses built-in templates and configured branding, not request-selected files.
