# Email Delivery

Target code: `packages/functions/auth/src/email`

## Owns

- Sending auth emails.
- Resend email delivery integration.
- Message template rendering.

## Must Not Own

- Verification or reset persistence.
- Verification or reset secret creation.
- OAuth token issuance.
- Product login, registration, or reset UI.

The email module sends messages only after the password provider/core has created a verification or reset secret.

## Runtime Provider

Resend is the only runtime email provider in the target core.

Required config in dev and production:

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
```

Optional config:

```text
AUTH_EMAIL_REPLY_TO
AUTH_EMAIL_BRAND_NAME
AUTH_EMAIL_SUPPORT_EMAIL
AUTH_EMAIL_VERIFY_SUBJECT
AUTH_EMAIL_RESET_SUBJECT
AUTH_EMAIL_VERIFY_TEMPLATE_PATH
AUTH_EMAIL_RESET_TEMPLATE_PATH
```

There is no `EMAIL_PROVIDER` switch and no console/log sender in runtime configuration. Tests may use an in-memory or mock sender internally, but that is not a deployable provider.
