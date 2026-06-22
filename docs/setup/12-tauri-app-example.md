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
