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

Rate-limit counters use independent HMAC source/email/client/provider identities. Deploying this
key shape starts fresh quota windows; older transient counters expire under their existing TTL.
There is no dual read, backfill or table scan. Do not delete tables, rewrite account data or rotate
the lookup secret for this counter change. Policies are described in [stage setup](03-stage-config.md#rate-limits).

Historical `audit` rows in older AuthTables have no TTL and are outside CloudWatch retention.
This runtime writes new audit events only to Lambda output. Deployment does not migrate or delete
old rows; agree separately on any bounded cleanup with the operator. Do not scan or clear a table
as part of setup.

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

Local infra checks resolve both functions' finite retention and JSON/INFO application filters using
inert constructors. They do not establish deployed log delivery or role permissions. With deployment
approval, use the [smoke guide](../operations/smoke-test.md#audit-and-logging) to check the existing
function groups. No new audit resources, runtime secrets or CloudWatch SDK permissions are needed.

## Done When

- Discovery and JWKS return `200`.
- Admin routes reject unsigned requests.
- Password registration sends email through Resend.
