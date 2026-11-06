# Provider Domain Modules

Source: [packages/functions/auth/src/providers/mod.rs](../../../../../packages/functions/auth/src/providers/mod.rs).

## What

Google, Apple, password, verified-email authority and internal confirmation-delivery exports.

## Why

Identity validation and password policy do not belong in the route table.

## How

Declares provider helpers used by api/providers. The verified-email module binds authority to a
provider/identity after JWT validation. The confirmation module coordinates internal rate-limited
mailbox proof delivery through the typed store and email sender. These modules do not install hosted
UI or public admin routes.
