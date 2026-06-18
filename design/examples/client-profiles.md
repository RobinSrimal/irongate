# Example Client Profiles

Target code: future `packages/functions/auth/src/config/clients.rs` and `packages/functions/auth/src/core/clients.rs` changes.

## Profiles

Examples should use explicit OAuth client profiles:

```text
spa
native_mobile
native_desktop
web_confidential
```

These profiles are target design. They do not need to exist in runtime code until a later implementation slice.

## Rules

| Profile | Secret | PKCE | CORS | Redirects |
| --- | --- | --- | --- | --- |
| `spa` | No | Required | Required for browser token calls | Exact HTTPS or localhost dev callback |
| `native_mobile` | No | Required | Not relevant | Claimed HTTPS/app links preferred, custom scheme allowed |
| `native_desktop` | No | Required | Not relevant | Loopback redirect with dynamic port |
| `web_confidential` | Yes | Recommended or required by policy | Usually not needed for token endpoint | Exact HTTPS callback |

## Future Client Config Shape

Future `auth.clients.toml` should move beyond the current public/confidential distinction:

```toml
[[clients]]
client_id = "web-spa"
client_type = "spa"
redirect_uris = ["https://app.example.com/auth/callback"]
allowed_origins = ["https://app.example.com"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email", "offline_access"]
pkce_required = true
token_endpoint_auth_method = "none"

[[clients]]
client_id = "desktop"
client_type = "native_desktop"
redirect_uris = ["http://127.0.0.1/oauth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email", "offline_access"]
pkce_required = true
token_endpoint_auth_method = "none"
```

For `native_desktop`, the registered loopback URI omits the runtime port. Validation matches scheme, loopback host, and path while allowing a dynamic port.

## Security Invariants

- Browser and native clients are public clients.
- Public clients must use PKCE.
- Public clients cannot authenticate with client secrets.
- `allowed_origins` are for CORS and are not redirect URIs.
- Redirect URIs are exact unless the profile explicitly supports native desktop loopback dynamic ports.
- Wildcard redirect URIs are not allowed.
- `client_credentials` remains out of v1.
