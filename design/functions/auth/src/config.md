# Runtime State

Source: [packages/functions/auth/src/config.rs](../../../../packages/functions/auth/src/config.rs).

## What

The shared AppState and basic deployment, endpoint and rate-limit configuration.

## Why

Handlers share a typed store, validated runtime configuration and reusable provider/email clients.

## How

Config loads table and issuer settings; RateLimitConfig supplies fixed-window defaults. TokenConfig remains a separate older configuration type; live token issuance uses RuntimeAuthConfig.ttls.
