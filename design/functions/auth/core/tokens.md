# Tokens

Target code: `packages/functions/auth/src/core/tokens.rs`

## Owns

- Access token claim shape.
- ID token claim shape.
- Refresh token metadata shape.
- Token TTL policy.
- Token family rules.

## Target Behavior

Access tokens are signed JWTs and are not persisted.

ID tokens are signed JWTs issued for OpenID Connect-compatible flows when the granted scope includes `openid`. They are not persisted.

Refresh tokens are persisted only by HMAC lookup digest and rotate on every use.

User-facing logout revokes refresh-token state through `POST /oauth/revoke`. It stops subsequent
refreshes for that session but does not invalidate already-issued access JWTs.

Token lifetimes are config-based:

```text
AUTH_ACCESS_TOKEN_TTL_SECONDS
AUTH_REFRESH_TOKEN_TTL_SECONDS
AUTH_ID_TOKEN_TTL_SECONDS
```

The default access-token TTL is 1 hour. The default ID-token TTL is 1 hour. The default refresh-token TTL is 30 days.

In KMS signing mode, every signed JWT means a KMS signing request. If a token response returns both an access token and an ID token, that response needs two signatures. Shorter access-token TTLs increase refresh frequency and signing calls; longer TTLs reduce signing calls but increase leaked-token lifetime.

Token issuance requires an active account. Disabled or deleted accounts cannot receive new authorization codes, access tokens, ID tokens, refresh tokens, or refresh-token rotations.

## Access Token Validation Model

Access tokens are self-contained JWTs. Resource APIs validate them locally using:

```text
issuer
audience
expiry
signature
algorithm
key ID
scopes or authorization claims
```

Account disable/delete takes effect immediately for login, authorization-code issuance, token refresh, userinfo, and all new token issuance. Already-issued access tokens remain valid until their `exp` if a resource API validates only the JWT locally. Short access-token TTLs are the v1 mitigation.

## Claim Boundaries

Access tokens are for API authorization. They should include the stable `sub`, issuer, API/resource audience, expiry, granted scopes, and minimal authorization claims needed by APIs.

ID tokens are for the OAuth/OIDC client. Initial ID tokens should include `iss`, `sub`, `aud` as the client ID, `iat`, `exp`, and `nonce` when the authorize request supplied one. Profile or email claims should be included only when requested and allowed by scope.

Refresh responses may return an ID token, but do not have to. If a refresh response includes one, it should keep the same `iss`, `sub`, and `aud` as the original authentication and should omit `nonce`.

## Example Client Storage Guidance

Token storage belongs to client applications, not Irongate core.

Expected example posture:

- `web` stores refresh tokens server-side in the BFF and gives the browser only an HttpOnly Secure SameSite session cookie.
- `app` stores refresh tokens in OS keychain or credential manager and keeps access tokens in memory.
- Protected API routes accept access tokens only and never store user refresh tokens.

No browser storage pattern fully protects against malicious JavaScript running in the application origin, so the recommended web example does not store OAuth tokens in browser JavaScript-accessible storage.

## Security Invariants

- Access token TTL is short.
- ID token TTL is short.
- Refresh token TTL is bounded.
- Access token TTL is shorter than refresh token TTL.
- ID token audience is the OAuth client, not the API.
- Access token audience is the API/resource, not the OAuth client UI.
- Resource APIs validate access tokens locally.
- Initial ID-token nonce comes from the client authorize request, not the external Google/Apple provider nonce.
- Refresh token reuse is detectable.
- Refresh token rotation is atomic in the store.
- Refresh token revocation supports normal app logout.
- Token claims are minimal and predictable.
- Disabled or deleted accounts cannot receive new tokens.
- Already-issued access tokens expire naturally according to `exp`.
- ID tokens are not API authorization tokens.
- Resource APIs use access tokens, not refresh tokens or ID tokens.
