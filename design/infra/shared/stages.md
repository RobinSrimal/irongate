# Infrastructure Stages

Implementation: [stage configuration](stage-config.md) and
[sst.config.ts](../../../sst.config.ts).

## What And Why

Only `dev` and `production` are supported. Explicit stage selection separates development
credentials, removal policy and security settings from production; unknown stages fail rather
than silently inheriting dev defaults.

Non-secret configuration is version-controlled in `infra/shared/stage-config.ts`.
Runtime secrets are set separately with SST for the chosen account/stage. Local Cloudflare deploy
credentials belong in ignored `.env`, not stage configuration.

## Current Defaults

| Setting | Dev | Production |
| --- | --- | --- |
| AWS profile | `irongate-dev` | `irongate-prod` |
| Removal / protection | remove / false | retain / true |
| Token signing | local ES256 | KMS ES256 |
| Table encryption | AWS owned | customer-managed KMS |
| DEV_MODE | false | false |
| Email | Resend, operator-supplied sender and action URLs | Same |
| Google / Apple | Disabled, identifier fields empty | Same |
| Examples | Disabled | Disabled |
| Audit mode | `cloudwatch` | `cloudwatch` |

Profile names can be overridden by the documented SST profile environment variables. The setup
script changes project-specific naming defaults for a new template consumer.

The audit mode controls structured stdout security events in the existing Lambda logs. Both stages
use finite 30-day log retention by default; checked-in values and environment overrides are validated.
None suppresses audit emission, not ordinary operational/access logging.
See [audit behavior](../../functions/auth/observability/audit.md).

## Integration Values

Google client ID and Apple Services ID, team ID and key ID are non-secret fields. Google client
secret and Apple private key are SST secrets. Leave optional providers disabled until their
required configuration is complete.

The web Worker can infer its request origin or use an explicit base URL. Exact callback registration
remains required in `auth.clients.toml`. Domain provisioning, example enablement and auth issuer
selection are separate settings; none implies the others.
