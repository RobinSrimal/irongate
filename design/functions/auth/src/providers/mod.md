# Provider Domain Modules

Source: [packages/functions/auth/src/providers/mod.rs](../../../../../packages/functions/auth/src/providers/mod.rs).

## What

Google, Apple and password domain exports.

## Why

Identity validation and password policy do not belong in the route table.

## How

Declares provider helpers used by api/providers. These modules do not install hosted UI or public admin routes.
