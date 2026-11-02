# Auth Stage Configuration

Implementation: [config.rs](../src/config.md), [runtime config](../src/config/environment.md)
and [infra stage settings](../../../infra/shared/stage-config.md).

## What And Why

Dev and production use the same password verification and Resend delivery model. Environment
configuration supplies issuer, table, secrets and policy values; the infrastructure selects
account/profile, resources and stage defaults.

## How

SST accepts only `dev` and `production`; unknown names fail. Both checked-in stages pass
`DEV_MODE=false`. Resend sender and action URLs are required rather than replacing delivery
with logged email links.

CORS comes from configured client origins. Localhost redirects may be configured for development;
deploying a production stage does not by itself rewrite the client registry into a different file.
See [stage boundaries](../../../infra/shared/stages.md).
