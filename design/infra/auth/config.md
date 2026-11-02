# Infrastructure Settings

Source: [infra/auth/config.ts](../../../infra/auth/config.ts).

## What

Validated table/signing/audit modes, retention and runtime permission helpers.

## Why

Stage configuration and optional environment overrides need one deploy-time interpretation.

## How

Reads checked-in defaults, validates mode strings and supported CloudWatch day counts, and exports exact DynamoDB action lists without Scan. Signing permissions are limited to Sign/GetPublicKey on the configured ARN. This module configures the audit switch, not its runtime storage semantics.
