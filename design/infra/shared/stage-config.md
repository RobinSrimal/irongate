# Stage Configuration

Source: [infra/shared/stage-config.ts](../../../infra/shared/stage-config.ts).

## What

Non-secret dev and production configuration and strict stage selection.

## Why

A reusable checkout must not inherit another project's identities or deployment settings.

## How

Only dev and production are accepted. Sender/provider/domain values are empty and examples disabled in both. Dev selects AWS-owned table encryption/local signing; production selects customer table KMS/KMS signing. Secrets are references elsewhere, not values in this file; examples.app.enabled currently creates no resources.
