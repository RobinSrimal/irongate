# Deploy Auth Core

## Goal

Deploy the AWS auth core for a stage.

## Inputs Needed

- AWS SSO profile for the stage.
- Stage config reviewed.
- Required SST secrets set.
- `auth.clients.toml` configured.

## Existing Data

Account, password-reset, authorization-code, refresh-token and refresh-family records require
`credential_generation`. Records without it fail closed; there is no default-zero reader or automatic
backfill. Before deploying over a populated stage with older records, agree on a separate data
migration or disposable-stage reset. Do not remove tables or reinterpret existing credentials just
to make deployment pass. Fresh deployments create generation-bound records directly.

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
