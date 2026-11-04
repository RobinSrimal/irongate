# Infrastructure Settings

Source: [infra/auth/config.ts](../../../infra/auth/config.ts).

## What

Validated table/signing/audit modes, retention and runtime permission helpers.

## Why

Stage configuration and optional environment overrides need one deploy-time interpretation.

## How

Reads checked-in defaults and environment overrides. Both resolved audit modes and finite log-retention
day counts are validated, including checked-in fallbacks; unsupported values throw before deployment.
Empty overrides use the configured defaults. Retention defaults to 30 days in both stages. Exports
exact DynamoDB action lists without Scan; signing permissions are Sign/GetPublicKey on one ARN.
Function logging consumes the resolved retention and passes the audit mode to both runtimes.
