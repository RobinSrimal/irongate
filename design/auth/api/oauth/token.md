# Token Endpoint

Target code: `packages/functions/auth/src/api/oauth/token.rs`

## Owns

- Parse `/token` form requests.
- Authenticate or validate the OAuth client.
- Dispatch supported grant types.
- Return OAuth token responses.
- Return an ID token when the granted scope includes `openid`.

## Supported Grants

- `authorization_code`
- `refresh_token`

`client_credentials` is not supported in v1.

## OIDC Response Behavior

For authorization-code and refresh-token responses:

```text
always return access_token
return refresh_token when refresh is allowed and offline access policy is satisfied
return id_token on authorization-code exchange when the granted scope includes openid
refresh responses may return id_token, but are not required to
```

The ID token audience is the OAuth client ID. The access token audience is the API/resource audience configured for the client or auth service.

Access tokens are self-contained JWTs. The token endpoint does not persist access tokens and does not create server-side access-token state for later introspection.

User-facing logout is handled by `POST /oauth/revoke`, which revokes refresh-token state. The token endpoint itself only issues and rotates tokens.

## Security Invariants

- Authorization codes are consumed once.
- Client ID, redirect URI, and PKCE verifier must be validated before the authorization code is deleted, or validated atomically as part of the typed consume operation.
- Wrong client ID, redirect URI, or PKCE verifier must not burn an otherwise valid authorization code.
- The subject from an authorization code or refresh token must still reference an active account before new tokens are issued.
- Disabled or deleted accounts cannot receive new access tokens.
- Public token endpoint rate-limit buckets include the declared client ID plus trusted API Gateway source identity so one caller cannot globally throttle a public client.
- Refresh tokens rotate atomically on every use.
- Refresh token reuse is detected and handled.
- Initial ID tokens are signed with the configured signing mode and include `iss`, `sub`, `aud`, `iat`, `exp`, and the client `nonce` when supplied on the authorize request.
- If refresh responses return an ID token, `iss`, `sub`, and `aud` must match the original authentication and `nonce` should be omitted.
- Raw refresh tokens must not be stored in DynamoDB keys.
- Access tokens are not stored for introspection or server-side revocation in v1.
- Browser and native public clients must use PKCE instead of client-secret authentication.
- Browser CORS origins are validated separately from OAuth redirect URIs once profile-aware CORS is implemented.

## Store Operations

- `get_authorization_code`
- `delete_authorization_code_if_current`
- `require_active_account`
- `create_refresh_token`
- `rotate_refresh_token`
- `revoke_refresh_token_family`

## Config Dependencies

- Read-only client registry lookup by `client_id`.
- Confidential client secret verification against configured secret material.
- `AUTH_ACCESS_TOKEN_TTL_SECONDS`.
- `AUTH_ID_TOKEN_TTL_SECONDS`.
- `AUTH_REFRESH_TOKEN_TTL_SECONDS`.
- `AUTH_AUTH_CODE_TTL_SECONDS` through authorization-code validation.
