# Tauri App Example

## Goal

Run the desktop app example against a deployed Irongate stage.

## Inputs Needed

- Deployed Irongate issuer URL.
- Native desktop OAuth client in `auth.clients.toml`.
- App dependencies installed.

## Files To Edit

- `auth.clients.toml`
- `packages/examples/app` environment/config files as documented by the app package.

## Client Requirements

The app client uses:

```text
client_type = "native_desktop"
pkce_required = true
token_endpoint_auth_method = "none"
```

It uses loopback redirect with dynamic port matching.

## Run

From `packages/examples/app`, set the actual issuer in the launch environment:

```bash
IRONGATE_ISSUER_URL="https://<your-auth-host>" IRONGATE_CLIENT_ID="app" npm run tauri dev
```

The default issuer is a reserved placeholder, not a deployed service. Refresh tokens stay in the
Rust-managed keychain; access and ID tokens are returned to frontend memory. This package implements
desktop callbacks, not mobile platform login. See [current app boundaries](../../design/examples/app.md).

## Validation

Run the app, then test:

- Password login.
- Google login if enabled.
- Apple login if enabled.
- Refresh after access-token expiry.
- Logout clears OS keychain token state.

## Done When

- The app completes login through Irongate.
- Refresh token state is stored in the OS keychain or credential manager.
