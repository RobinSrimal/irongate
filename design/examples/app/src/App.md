# Desktop View

Sources: [App.tsx](../../../../packages/examples/app/src/App.tsx) and [App.css](../../../../packages/examples/app/src/App.css).

## What

The React sign-in/session view and its layout/style rules.

## Why

The optional native example needs a small interface to exercise password/provider login, refresh and logout.

## How

App owns form values, busy/status state and the returned AppSession in React memory, calling the Tauri bridge for auth actions. It shows keychain-session restoration when available and removes local session state after successful logout. CSS defines the panel, forms, provider actions, badges and responsive styling. Neither file implements OS credential storage; access/ID tokens are present in React state, while refresh tokens stay in Rust.
