# OAuth API

Target code: `packages/functions/auth/src/api/oauth`

## Owns

- OAuth endpoint handlers.
- OAuth request and response DTOs.
- Protocol-level error responses.

## Endpoints

- `GET /authorize`
- `POST /token`
- `GET /userinfo`
- `GET /.well-known/oauth-authorization-server`
- `GET /.well-known/jwks.json`

## Security Invariants

- Exact redirect URI matching.
- PKCE for authorization-code clients.
- Single-use authorization codes.
- Refresh token rotation.
- Minimal claims in userinfo.
