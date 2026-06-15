# Resend

Target code: `packages/functions/auth/src/email/resend.rs`

## Owns

- Resend API request construction.
- Resend API response parsing.
- Mapping Resend failures to email delivery errors.

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
```

## Sender Verification

Sender/domain verification is handled by Resend and DNS configuration, not by auth code.

The auth service should assume that `AUTH_EMAIL_FROM` has been configured correctly with Resend. If Resend rejects a send request, registration or reset should remain pending and the user must not be marked verified.

## Security Invariants

- Never log `RESEND_API_KEY`.
- Never log full verification or reset links.
- Do not include password hashes, refresh tokens, or auth codes in email content.
- Delivery success is not account verification.
- Only consuming the verification secret marks the user verified.

## Failure Behavior

- Registration may create an unverified user before email send.
- If verification email delivery fails, return a safe error and allow retry/resend under rate limits.
- Password reset request should avoid revealing whether an email exists.
- Resend API failures should be logged with coarse reason and provider request ID if available, not with message secrets.
