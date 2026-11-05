# Provider API Modules

Source: [packages/functions/auth/src/api/providers/mod.rs](../../../../../../packages/functions/auth/src/api/providers/mod.rs).

## What

Password, Google and Apple HTTP handler modules.

## Why

Request parsing and provider-specific handoffs stay separate from the public router.

## How

Declares the three public handler modules and a private [issuance error mapper](issuance.md) shared
by Google and Apple callbacks. Handlers use domain helpers and AuthStore; provider credentials
come from shared runtime configuration.
