# Audit Mode

Source: [packages/functions/auth/src/config/audit.rs](../../../../../packages/functions/auth/src/config/audit.rs).

## What

Parsing of cloudwatch and none, defaulting to cloudwatch.

## Why

Both Lambda entrypoints use the same explicit audit enablement switch.

## How

Unknown values fail parsing. Cloudwatch selects the production structured stdout emitter; none
touches neither audit output nor fallback. Ordinary tracing and API Gateway logging are independent.
The mode does not select a storage adapter or disable CloudWatch log groups.
