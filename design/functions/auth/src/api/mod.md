# API Modules

Source: [packages/functions/auth/src/api/mod.rs](../../../../../packages/functions/auth/src/api/mod.rs).

## What

Public module declarations for OAuth, providers and shared admin handlers.

## Why

HTTP entrypoints are grouped independently of their deployed router.

## How

Declares admin, oauth and providers. The admin crate explicitly mounts the admin router; exporting the module does not expose admin paths on the public router.
