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

## Audit And Operational Logs

Set `infra.auditLogMode = "cloudwatch"` (default) to emit bounded structured audit events through
both existing Lambda application logs. `"none"` suppresses audit output and its failure diagnostics,
not ordinary Lambda tracing or API Gateway access logs. No audit service or additional secret is
required. `AUTH_AUDIT_LOG_MODE` can override the stage setting.

`infra.logRetentionDays` defaults to 30 for both stages; `AUTH_LOG_RETENTION_DAYS` overrides it.
Both checked-in values and overrides must be supported finite CloudWatch day counts (for example
7, 14, 30 or 90). Both functions retain JSON/INFO application logging and SST-managed log groups and
roles. Ordinary log ingestion/storage charges apply; none mode does not eliminate operational logs.

Events contain configured client IDs, internal subjects, HMAC token references and trusted IAM
operator identifiers where applicable. These are identifying correlation data. Restrict log access;
never paste event contents or bearer values into public setup notes. A local emission problem
attempts a safe `audit_emit_failed` ERROR record without the failed payload or underlying error.

Delivery is best effort, not a transactional or exactly-once audit ledger. Existing DynamoDB audit
rows from older deployments remain untouched without TTL; CloudWatch retention does not cover them.
See [audit behavior and limits](../../design/functions/auth/observability/audit.md).

## Rate Limits

Endpoint policy defaults live in `packages/functions/auth/src/config/rate_limits.rs`, not stage
config. There are no new environment variables or secrets; identifiers use the existing HMAC lookup
secret. See the [policy table](../../design/functions/auth/store/rate-limits.md).

Password register/login/forgot apply independent source and normalized-email budgets. OAuth and
provider starts have aggregate source ceilings plus narrower client/session budgets. Invalid enabled
policies fail closed. These are starting thresholds, not measured production capacity.

Users behind a NAT or BFF share its outbound IP budget. API Gateway sees a Cloudflare BFF's source,
not each browser user's address. Do not solve throttling by trusting forwarded headers. Review shared
egress traffic and email-target throttling trade-offs before adjusting the Rust policies and deploying.

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
