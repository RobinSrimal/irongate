# Audit Mode

Source: [packages/functions/auth/src/config/audit.rs](../../../../../packages/functions/auth/src/config/audit.rs).

## What

Parsing of cloudwatch and none, defaulting to cloudwatch.

## Why

Both Lambda entrypoints use the same audit enablement switch.

## How

Unknown values fail parsing. The mode name does not describe the current storage backend accurately: src/audit.rs persists enabled events in DynamoDB, not structured CloudWatch audit output.
