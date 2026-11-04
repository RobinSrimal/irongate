# Runtime State

Source: [packages/functions/auth/src/config.rs](../../../../packages/functions/auth/src/config.rs).

## What

The shared AppState and basic deployment, endpoint and rate-limit configuration.

## Why

Handlers share a typed store, validated runtime configuration, reusable provider/email clients and
an explicit AuditEmitter. Logging does not depend on AuthStore.

## How

Config loads table and issuer settings and re-exports endpoint/limit/config types from
[rate_limits](config/rate_limits.md). That module supplies independent fixed-window policies.
TokenConfig remains a separate older configuration type; live token issuance uses RuntimeAuthConfig.ttls.
