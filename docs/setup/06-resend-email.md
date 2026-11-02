# Resend Email

## Goal

Configure password verification and password reset email delivery.

## Inputs Needed

- Resend API key.
- Verified sender/domain.
- Verification URL base.
- Reset URL base.

## Files To Edit

- `infra/shared/stage-config.ts` for non-secret sender and URL values.

## SST Secrets

```bash
npx sst secret set ResendApiKey "<resend api key>" --stage dev
```

## Stage Config

Set:

```text
email.from
email.verifyUrlBase
email.resetUrlBase
email.brandName optional
email.replyTo optional
```

The verification/reset URL bases must point to real handlers in the consuming app. The optional
web example handles verification only, not password reset. Template-path environment fields are
currently parsed but not loaded; email bodies use built-in templates with branding/subject settings.

## Validation

Register a password user and confirm a Resend email arrives. The verification link should point at
the configured app or web example URL.

## Done When

- Registration sends a verification email.
- Verification consumes the token once.
- Reset email links point at the intended app URL.
