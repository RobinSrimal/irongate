# Stage Config

## Goal

Edit checked-in, non-secret stage configuration for dev and production.

## Inputs Needed

- Public issuer URL or decision to use the generated API Gateway URL.
- Verification and reset URL bases.
- Email sender addresses.
- KMS choices for DynamoDB and signing.
- Optional Google and Apple non-secret IDs.
- Optional example enablement.

## Files To Edit

- `infra/shared/stage-config.ts`

## What Belongs Here

Checked-in non-secret values:

- `email.from`
- `email.verifyUrlBase`
- `email.resetUrlBase`
- `email.brandName`
- `auth.issuerUrl`
- `auth.googleClientId`
- `auth.apple.clientId`
- `auth.apple.teamId`
- `auth.apple.keyId`
- `signing.mode`
- `signing.keyId`
- `infra.tableKmsMode`
- `infra.auditLogMode`
- `infra.logRetentionDays`
- `examples.enabled`
- `examples.web.enabled`

## What Does Not Belong Here

Secrets:

- Resend API key.
- HMAC lookup secret.
- Local ES256 private key.
- Google client secret.
- Apple private key.
- OAuth confidential client secrets.

Set those with SST secrets instead.

## Validation

```bash
npm run typecheck
npm run test:infra
```

## Common Failures

- Putting provider secrets in stage config.
- Setting dev email links to localhost while testing deployed Cloudflare examples.
- Enabling Apple without setting the `ApplePrivateKey` SST secret.
- Using KMS signing in dev when local signing is intended to avoid KMS signing cost.

## Done When

- Stage config contains only reviewed non-secret settings.
- `npm run typecheck` passes.
