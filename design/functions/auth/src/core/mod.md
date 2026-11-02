# Core Modules

Source: [packages/functions/auth/src/core/mod.rs](../../../../../packages/functions/auth/src/core/mod.rs).

## What

Exports of client, password, scope, subject and token helpers.

## Why

Protocol data and policies can be used independently of the HTTP router.

## How

The module declares five submodules. Account lifecycle and identity persistence currently live in store/mod.rs rather than additional core modules.
