# OAuth Client Configuration

Target code: `packages/functions/auth/src/config/clients.rs`

## Owns

- Loading OAuth client definitions.
- Validating client configuration at startup.
- Supplying client config to the auth core.

## Decision

The first version uses config-only OAuth clients.

Config-only means the source of truth for OAuth clients is repo/deployment configuration. V1 uses a checked-in TOML client config file for non-secret client settings and SST secrets for actual secret values in deployed stages. Clients are changed by editing config and redeploying.

The auth Lambda may load those definitions at startup and keep them in memory. It should not expose runtime APIs that create, update, disable, rotate, or delete clients.

This removes the need for:

- Public admin bootstrap.
- Custom runtime admin API keys.
- Client creation endpoints in the auth Lambda.
- Client scans or control-plane writes in the auth table.

## Target Config Shape

Clients are declared in `auth.clients.toml`.

Current required fields:

```text
client_id
client_type
redirect_uris
allowed_grant_types
allowed_scopes
pkce_required
token_endpoint_auth_method
```

V1 currently distinguishes public and confidential clients. The example architecture introduces a future profile split:

```text
spa
native_mobile
native_desktop
web_confidential
```

Those profiles should remain config-only and should be added in a later implementation slice. See `design/examples/client-profiles.md`.

Confidential clients also need a deployment secret reference. In deployed stages, that reference resolves to an SST secret binding. The runtime should verify only a secret hash derived from that deployment secret; plaintext client secrets must not be stored in DynamoDB.

Example:

```toml
[[clients]]
client_id = "web"
client_type = "public"
redirect_uris = ["https://app.example.com/auth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email", "offline_access"]
pkce_required = true
token_endpoint_auth_method = "none"

[[clients]]
client_id = "backend"
client_type = "confidential"
client_secret_ref = "AUTH_CLIENT_BACKEND_SECRET"
redirect_uris = ["https://api.example.com/auth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email", "offline_access"]
pkce_required = false
token_endpoint_auth_method = "client_secret_basic"
```

## Runtime Boundary

The runtime client registry is read-only after startup. Route code can ask for a client by `client_id`, but it cannot persist client changes.

Allowed:

- Validate configured clients at startup.
- Fail startup when a configured client is invalid.
- Read a configured client by exact `client_id`.
- Verify a confidential client secret against a derived hash.
- Resolve client secret refs from SST secrets or local environment variables.

Not allowed in v1:

- `POST /admin/clients`.
- Runtime client-secret rotation.
- Runtime redirect URI updates.
- Table scans to discover clients.
- External mutation of clients through the auth API.

## Future Example Client Profiles

When example support is implemented, client config should express browser/native behavior directly:

```text
client_type = "spa"
client_type = "native_mobile"
client_type = "native_desktop"
client_type = "web_confidential"
```

Browser clients should also define CORS origins separately from redirect URIs:

```text
allowed_origins = ["https://app.example.com"]
```

Redirect URIs are OAuth callback destinations. Allowed origins are browser CORS policy inputs for endpoints such as `/token`, `/userinfo`, and `/oauth/revoke`.

## Security Invariants

- Redirect URIs are exact-match only.
- Public clients require PKCE.
- Confidential client secrets are never stored plaintext.
- Confidential client secret refs are names only, never raw secret values.
- Disabled or invalid clients fail startup rather than partially deploying.
- Runtime auth routes cannot create or mutate clients in the target core.
- DynamoDB is not the source of truth for OAuth clients in v1.
- Clients must explicitly allow every scope they can receive.
- Browser and native client secrets are not trusted, because they cannot keep shared secrets.
- Wildcard redirect URIs are not allowed.
- Native desktop loopback dynamic-port matching must be profile-limited.

## Out Of V1

`client_credentials` is not supported in v1. Adding machine-to-machine tokens later requires a separate design for confidential-client scope policy, audience rules, and token claims.
