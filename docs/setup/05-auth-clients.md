# Auth Clients

## Goal

Configure OAuth clients that are allowed to use Irongate.

## Inputs Needed

- Client ID.
- Client type.
- Redirect URIs.
- Allowed scopes.
- Allowed origins for browser clients.
- Confidential client secret reference, if applicable.

## Files To Edit

- `auth.clients.toml`

## Client Types

Use explicit profiles:

```text
spa
native_mobile
native_desktop
web_confidential
```

The recommended web example uses `web_confidential` through a BFF. The Tauri app uses
`native_desktop`.

## Validation

```bash
npm run typecheck
npm run deploy -- --stage dev
```

Startup fails when the client file is invalid.

## Done When

- Every redirect URI is exact, except native desktop loopback dynamic ports.
- Browser clients have exact `allowed_origins`.
- Public clients require PKCE.
- Confidential clients reference secrets by name only.
