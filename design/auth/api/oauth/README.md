# OAuth API

Target code: `packages/functions/auth/src/api/oauth`

## Owns

- OAuth endpoint handlers.
- OAuth request and response DTOs.
- Protocol-level error responses.

## Endpoints

- `GET /authorize`
- `POST /token`
- `POST /oauth/revoke`
- `GET /userinfo`
- `GET /.well-known/openid-configuration`
- `GET /.well-known/oauth-authorization-server`
- `GET /.well-known/jwks.json`

## Out Of V1

- Token introspection endpoint.
- Opaque access tokens.
- Server-side access-token revocation.

## Security Invariants

- Exact redirect URI matching.
- PKCE for authorization-code clients.
- Single-use authorization codes.
- ID token issuance for authorization-code flows that include `openid`.
- Self-contained JWT access tokens validated locally by resource APIs.
- Refresh token rotation.
- Refresh token revocation for user-facing logout.
- Minimal claims in userinfo.
