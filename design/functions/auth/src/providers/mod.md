# Provider Domain Modules

Source: [packages/functions/auth/src/providers/mod.rs](../../../../../packages/functions/auth/src/providers/mod.rs).

## What

Google, Apple, password and shared verified-email authority exports.

## Why

Identity validation and password policy do not belong in the route table.

## How

Declares provider helpers used by api/providers. The verified-email module binds authority to a
provider/identity after JWT validation. These modules do not install hosted UI or public admin routes.
