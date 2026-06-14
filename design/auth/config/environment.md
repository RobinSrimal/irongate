# Environment Configuration

Target code: `packages/functions/auth/src/config/environment.rs`

## Owns

- Runtime environment variable parsing.
- Required/optional setting validation.
- Typed config structs for auth modules.

## Target Setting Families

- Issuer URL.
- Enabled providers.
- OAuth client definitions or client config source.
- Password policy and email verification settings.
- Resend email delivery settings.
- Google and Apple credentials.
- HMAC lookup secret reference.
- Token TTLs.
- Rate-limit settings.

## Security Invariants

- Startup fails in every stage when `RESEND_API_KEY` or `AUTH_EMAIL_FROM` is missing.
- `DEV_MODE` is explicit and stage-limited.
- Secrets are not printed in validation errors.

## Required Email Config

The target core has one email config shape for dev and production:

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
AUTH_EMAIL_REPLY_TO optional
```

There is no `EMAIL_PROVIDER` setting in the target core.
