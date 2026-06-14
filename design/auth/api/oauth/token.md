# Token Endpoint

Target code: `packages/functions/auth/src/api/oauth/token.rs`

## Owns

- Parse `/token` form requests.
- Authenticate or validate the OAuth client.
- Dispatch supported grant types.
- Return OAuth token responses.

## Supported Grants

- `authorization_code`
- `refresh_token`
- optionally `client_credentials`

## Security Invariants

- Authorization codes are consumed once.
- PKCE verifier must match the stored challenge.
- Client ID and redirect URI must match the stored authorization code.
- Refresh tokens rotate atomically on every use.
- Refresh token reuse is detected and handled.
- Raw refresh tokens must not be stored in DynamoDB keys.

## Store Operations

- `take_authorization_code`
- `create_refresh_token`
- `rotate_refresh_token`
- `revoke_refresh_family`
