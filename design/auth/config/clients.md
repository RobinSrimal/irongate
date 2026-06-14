# OAuth Client Configuration

Target code: `packages/functions/auth/src/config/clients.rs`

## Owns

- Loading OAuth client definitions.
- Validating client configuration at startup.
- Supplying client config to the auth core.

## Decision

The first version uses deployment-defined clients, not runtime admin-created clients.

This removes the need for:

- Public admin bootstrap.
- Runtime admin API keys.
- Client creation endpoints in the auth Lambda.

## Target Config Shape

Clients can be declared through environment/SST config in a structured form.

Required fields:

```text
client_id
client_type
redirect_uris
allowed_grant_types
allowed_scopes
pkce_required
token_endpoint_auth_method
```

Confidential clients also need a secret source. Runtime storage should contain only a secret hash, or the secret hash should be derived at startup from a deployment secret.

## Security Invariants

- Redirect URIs are exact-match only.
- Public clients require PKCE.
- Confidential client secrets are never stored plaintext.
- Disabled or invalid clients fail startup rather than partially deploying.
- Runtime auth routes cannot create or mutate clients in the target core.

## Open Decision

`client_credentials` is optional for v1. If included, it should be limited to confidential clients and explicitly configured per client.
