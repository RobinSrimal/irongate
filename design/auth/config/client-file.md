# Client Config File

Target file: `auth.clients.toml`

Target code: `packages/functions/auth/src/config/clients.rs`

## Decision

OAuth client definitions live in a checked-in TOML file. Secret values do not.

The file contains non-secret, reviewable client settings:

- Client IDs.
- Client type.
- Redirect URIs.
- Allowed grant types.
- Allowed scopes.
- PKCE policy.
- Token endpoint auth method.
- Secret reference names for confidential clients.

Actual secret values are supplied separately through SST secrets in deployed stages, or local environment variables in local development.

## Example

```toml
[[clients]]
client_id = "web"
client_type = "public"
redirect_uris = ["http://localhost:3000/auth/callback"]
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

## Secret Resolution

For a confidential client:

```text
client_secret_ref = "AUTH_CLIENT_BACKEND_SECRET"
```

The runtime resolves that reference from its configured secret source:

```text
deployed stage -> SST secret binding
local dev -> environment variable
```

The client config parser must never treat `client_secret_ref` as the secret value itself. It is only a lookup name.

## Validation

Startup should fail when:

- The file is missing or malformed.
- A client ID is duplicated.
- A redirect URI is invalid.
- A public client has a client secret.
- A confidential client is missing `client_secret_ref`.
- A referenced secret is not available.
- A client allows unsupported grants or scopes.
- A client uses `client_credentials` in v1.
- A public authorization-code client does not require PKCE.

## Security Invariants

- Raw client secrets are never committed to the repo.
- Raw client secrets are never stored in DynamoDB.
- Secret refs are exact names, not user-controlled request input.
- Client definitions are loaded and validated at startup.
- Client changes require config change plus redeploy.
