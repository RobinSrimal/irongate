# Example Client Profiles

Target code: `packages/functions/auth/src/config/clients.rs` and `packages/functions/auth/src/core/clients.rs`.

## Profiles

Examples use explicit OAuth client profiles:

```text
native_mobile
native_desktop
web_confidential
```

The auth core also supports `spa` for public browser clients, but the recommended web example uses `web_confidential` with a BFF so refresh tokens never enter browser JavaScript. Legacy `public` and `confidential` values may parse as aliases, but example config should use the explicit profiles.

## Rules

| Profile | Secret | PKCE | CORS | Redirects |
| --- | --- | --- | --- | --- |
| `native_mobile` | No | Required | Not relevant | Claimed HTTPS/app links preferred, reverse-domain custom scheme allowed |
| `native_desktop` | No | Required | Not relevant | Loopback redirect with dynamic port |
| `web_confidential` | Yes | Recommended or required by policy | Usually not needed for token endpoint | Exact HTTPS callback |

## Client Config Shape

`auth.clients.toml` uses explicit profiles:

```toml
[[clients]]
client_id = "web"
client_type = "web_confidential"
client_secret_ref = "AUTH_CLIENT_WEB_SECRET"
redirect_uris = ["https://app.example.com/auth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email", "offline_access"]
pkce_required = true
token_endpoint_auth_method = "client_secret_basic"

[[clients]]
client_id = "app"
client_type = "native_desktop"
redirect_uris = ["http://127.0.0.1/oauth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email", "offline_access"]
pkce_required = true
token_endpoint_auth_method = "none"
```

For `native_desktop`, the registered loopback URI omits the runtime port. Validation matches scheme, loopback host, and path while allowing a dynamic port.

## Security Invariants

- Native app clients are public clients.
- Native public clients must use PKCE.
- Native public clients cannot authenticate with client secrets.
- Web browser clients should use the BFF example instead of storing OAuth tokens in browser JavaScript.
- `allowed_origins` are for CORS and are not redirect URIs.
- CORS responses use exact configured origins, never wildcard origins.
- Redirect URIs are exact unless the profile explicitly supports native desktop loopback dynamic ports.
- Wildcard redirect URIs are not allowed.
