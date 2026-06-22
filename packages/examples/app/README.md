# Irongate App Example

Desktop-first Tauri example for Irongate.

The app talks directly to the deployed Irongate auth API and demonstrates native-client auth without
Cloudflare, a BFF, or browser refresh-token storage.

## Auth Shape

- Public OAuth client: `app`
- Redirect URI: `http://127.0.0.1:<dynamic-port>/oauth/callback`
- OAuth flow: Authorization Code + PKCE
- Identity providers: password, Google, and Apple
- Refresh-token storage: OS keychain through the Rust Tauri backend
- Access-token storage: React memory only

The app opens Google and Apple login in the external system browser. Password login is submitted from
the app UI to Irongate and still uses the same PKCE code exchange.

## Before Running

Deploy Irongate after adding the `app` client in the root `auth.clients.toml`; the deployed auth API
must know about the native desktop client before login can succeed.

The app defaults to the current dev issuer:

```text
https://1e88qilxk6.execute-api.eu-west-1.amazonaws.com
```

Override these values from the shell when needed:

```sh
IRONGATE_ISSUER_URL=https://auth.example.com \
IRONGATE_APP_CLIENT_ID=app \
IRONGATE_APP_SCOPE="openid email offline_access" \
npm --workspace @irongate/example-app run tauri dev
```

## Commands

```sh
npm --workspace @irongate/example-app test
cargo check --manifest-path packages/examples/app/src-tauri/Cargo.toml
npm --workspace @irongate/example-app run tauri dev
```

## Mobile Notes

This package is desktop-first. A future mobile adaptation should keep the same Irongate protocol
shape but use mobile platform redirects and platform secure storage instead of desktop loopback and
desktop keychain behavior.
