# OAuth Protocol Modules

Source: [packages/functions/auth/src/oauth/mod.rs](../../../../../packages/functions/auth/src/oauth/mod.rs).

## What

Authorize, PKCE, token, userinfo, revocation and metadata implementations.

## Why

Protocol behaviour is separated from Axum router assembly.

## How

The module declares the six implementations. api/oauth re-exports their mounted handlers; it is not a separate implementation.
