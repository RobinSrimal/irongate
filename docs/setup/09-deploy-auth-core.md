# Deploy Auth Core

## Goal

Deploy the AWS auth core for a stage.

## Inputs Needed

- AWS SSO profile for the stage.
- Stage config reviewed.
- Required SST secrets set.
- `auth.clients.toml` configured.

## Commands

```bash
unset AWS_PROFILE
npm run typecheck
npm run test:infra
npm run deploy -- --stage dev
```

Production:

```bash
npm run deploy -- --stage production
```

## Outputs

SST prints:

```text
ApiUrl
ApiId
TableName
TableKmsKeyArn
SigningKmsKeyArn
AdminRouteArnPattern
```

## Validation

```bash
curl "<ApiUrl>/.well-known/openid-configuration"
curl "<ApiUrl>/.well-known/jwks.json"
```

## Done When

- Discovery and JWKS return `200`.
- Admin routes reject unsigned requests.
- Password registration sends email through Resend.
