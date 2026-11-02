# App Example

Implementation: [React UI](app/src/App.md), [Rust auth commands](app/src-tauri/src/auth.md),
[Tauri setup](app/src-tauri/src/lib.md) and [package README](app/README.md).

## What And Why

A desktop-first Tauri app uses React/TypeScript for forms and Rust for HTTP, PKCE, loopback callbacks
and keychain access. It exercises the public auth API without depending on the Cloudflare BFF or
reading DynamoDB.

The registered client is `native_desktop`, with mandatory PKCE and no client secret.
Configure `IRONGATE_ISSUER_URL` explicitly: the default is the reserved
`https://auth.example.com` placeholder. `IRONGATE_CLIENT_ID` defaults to `app`.

## Login And Sessions

Password login forwards credentials through the authorize/password/code-exchange flow.
Google and Apple open the external browser, then return to a loopback listener with a dynamic port.
The app checks callback path/state and exchanges the code with its PKCE verifier.

Refresh tokens are stored through the Rust `keyring` dependency. Access and ID tokens are returned
over Tauri IPC and held in frontend memory; they are not confined to the Rust process.
Restore/refresh commands use the stored refresh token and replace it after a successful rotation.
Logout attempts remote revocation and deletes the local keychain entry.

## Boundaries And Limits

- Browser provider login is external, not embedded in the webview.
- The package contains desktop loopback behavior, not implemented iOS/Android login integrations.
- OS credential-store availability depends on the dependency's enabled backend and the host.
  This repository's build/tests do not certify every desktop platform.
- Refresh/logout commands are not a single serialized session state machine; there is no persistent
  marker for an uncertain refresh outcome.
- The UI's busy state is an interaction guard, not a cross-command concurrency guarantee.
- Tauri's checked-in CSP setting is null. The example is not a hardened webview security boundary.
- TypeScript helper tests do not exercise real browser callbacks, keychain prompts or device login.

Native application identity and packaging belong to the consuming application. No separate app
cloud resources are created by `infra/examples`; see [app infrastructure](../infra/examples/app.md).
