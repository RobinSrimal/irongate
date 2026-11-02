# Tauri Auth Bridge

Source: [packages/examples/app/src/auth/tokenStore.ts](../../../../../packages/examples/app/src/auth/tokenStore.ts).

## What

Typed frontend invoke wrappers for login, refresh, logout and stored-session presence.

## Why

React does not directly access the operating-system credential store.

## How

Calls registered Rust commands and returns AppSession. The name does not mean it stores tokens in browser storage; access/ID tokens returned by Rust remain in frontend memory.
