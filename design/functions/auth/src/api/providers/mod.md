# Provider API Modules

Source: [packages/functions/auth/src/api/providers/mod.rs](../../../../../../packages/functions/auth/src/api/providers/mod.rs).

## What

Password, Google and Apple HTTP handler modules.

## Why

Request parsing and provider-specific handoffs stay separate from the public router.

## How

Declares the three modules; their handlers use domain helpers and AuthStore. Provider credentials are obtained from shared runtime configuration.
