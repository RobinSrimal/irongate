# Web SPA Example

Target code: `packages/examples/web-spa`

## Owns

- Static browser application.
- PKCE verifier/challenge generation.
- OAuth redirect handling.
- Calling a protected resource API with access tokens.
- Minimal token handling for a browser-only client.

## Client Profile

The web SPA is a public client:

```text
client_type = "spa"
token_endpoint_auth_method = "none"
pkce_required = true
```

It must not use or embed a client secret.

## Redirects And Origins

- Production redirect URIs are exact HTTPS URLs.
- Local development may use localhost.
- Wildcard redirect URIs are not allowed.
- Browser calls to `/token`, `/userinfo`, and `/oauth/revoke` require explicit configured CORS origins.

Future client config should distinguish:

```text
redirect_uris = [...]
allowed_origins = [...]
```

Redirect URIs handle OAuth callbacks. Allowed origins handle browser CORS.

## Token Storage

Preferred browser posture:

- Keep access tokens in memory.
- Avoid localStorage for tokens.
- Avoid refresh tokens in browser storage where possible.
- If refresh tokens are used, require rotation, reuse detection, and clear documentation of browser XSS risk.

No browser token-storage pattern fully protects against malicious JavaScript running in the app origin. The example should say that plainly.

## Security Invariants

- Authorization Code with PKCE only.
- No implicit flow.
- No resource-owner password grant.
- No client secret.
- Validate `state`.
- Use `nonce` for OIDC login.
- Remove authorization codes from browser-visible URLs after handling.
- Avoid third-party scripts on callback/auth-sensitive pages.
