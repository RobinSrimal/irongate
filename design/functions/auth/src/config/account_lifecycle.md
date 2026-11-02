# Lifecycle Configuration

Source: [packages/functions/auth/src/config/account_lifecycle.rs](../../../../../packages/functions/auth/src/config/account_lifecycle.rs).

## What

Deleted-identity reuse policy and retention validation.

## Why

Re-registration policy must be explicit without recycling old subject identifiers.

## How

Parses immediate, after_retention or never. The default is after_retention with 30 days; that mode requires a retention between 1 and 3650 days.
