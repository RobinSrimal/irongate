# Resend Email Delivery

Target code: `packages/functions/auth/src/email/delivery.rs`

## Owns

- Resend implementation.
- Delivery calls for verification and password reset emails.
- Small internal trait for tests, if needed.

## Target Interface

```text
send_verification(to, rendered_message) -> delivery_id
send_password_reset(to, rendered_message) -> delivery_id
```

## Required Runtime Config

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
AUTH_EMAIL_REPLY_TO optional
```

The same config shape is used in dev and production.

## Security Invariants

- Runtime email delivery uses Resend only.
- There is no console, log, SMTP, SES, or provider switch in the target core.
- Resend API key comes from secrets.
- Delivery errors must not mark users verified or reset passwords.
- Logs must not include full verification or reset codes.

## Postponed

SMTP, SES, and alternate email providers are postponed. Adding them later requires a separate design because each adds configuration, deliverability, and failure-mode complexity.
