# Clients

Target code: `packages/functions/auth/src/core/clients.rs`

## Owns

- OAuth client model.
- Redirect URI validation.
- Grant type validation.
- Client secret verification rules.
- Client profile rules for browser, native, and confidential clients.

## Target Behavior

The first narrow template uses config-only OAuth clients. The core receives a validated, read-only client registry from configuration and applies OAuth rules against that registry.

Runtime client management is out of v1. Adding it later would require a separate client-management control-plane design. The IAM-protected account lifecycle admin routes must not mutate OAuth clients.

## Client Profiles

Example applications require more specific client profiles than the older public/confidential split:

```text
spa
native_mobile
native_desktop
web_confidential
```

Rules:

- `spa`, `native_mobile`, and `native_desktop` are public clients.
- Public clients require PKCE.
- Browser and native clients cannot keep shared client secrets.
- `spa` uses exact redirect matching and configured CORS origins.
- `native_mobile` uses claimed HTTPS links or private-use custom schemes.
- `native_desktop` may use loopback redirects with dynamic runtime ports.
- `web_confidential` may authenticate at the token endpoint with a deployment secret.

See `design/examples/client-profiles.md`.

## Security Invariants

- Redirect URI matching is exact.
- Public clients cannot use client secrets.
- Confidential clients store only secret hashes.
- Client credentials grant is not supported in v1.
- Disabled clients cannot receive tokens.
- Client definitions cannot be created or changed through public auth routes.
- Dynamic-port redirect matching is allowed only for native desktop loopback redirects.
