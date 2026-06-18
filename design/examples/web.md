# Web Example

Target code: `packages/examples/web`

## Owns

- Browser web application example.
- BFF session handling.
- OAuth callback handling for the web client.
- Server-side refresh-token storage.
- Password registration, email verification, login, callback, signed-in page, and logout.

## Pattern

The web example uses a Backend-for-Frontend pattern:

```text
browser
  -> web BFF
  -> Irongate authorize/token/refresh/revoke
  -> signed-in browser session
```

The browser receives only an application session cookie:

```text
HttpOnly
Secure
SameSite=Lax or Strict depending on route shape
```

The browser must not receive refresh tokens. Frontend JavaScript must not read access tokens, refresh tokens, authorization codes after callback handling, or client secrets.

The Worker derives its public base URL from the incoming request origin by default. `WEB_BASE_URL`
is an optional override for local tunnels, custom domains, or unusual proxy setups. Irongate still
requires the active callback URL to be listed in `auth.clients.toml`.

The request-origin fallback exists to avoid first-deploy friction with a generated `workers.dev`
URL. Production examples should use an explicit domain and exact Irongate client registration:

```text
https://auth-demo.example.com/auth/callback
```

For production hardening, the Worker should also enforce an allowed-origin list before starting
OAuth. Unexpected origins should fail before the Worker calls Irongate.

## OAuth Flow

1. Browser starts login at the web BFF.
2. BFF creates OAuth state and PKCE verifier.
3. BFF starts Irongate `/authorize` with `provider=password`.
4. BFF captures the Irongate password authorize-session key and renders its own password form.
5. BFF posts the password credentials to Irongate `/password/login`.
6. Irongate redirects back to the BFF callback with an authorization code.
7. BFF validates state and exchanges the code at Irongate `/token`.
8. BFF stores the refresh token server-side under an opaque application session ID.
9. BFF sets the browser session cookie.
10. BFF renders a minimal signed-in page.

## Protected API Routes

The first web example does not need business data or protected `/api/*` routes.
The later Security Lab slice may add routes such as:

```text
GET /api/me
```

Those routes should validate session state for browser calls. If direct bearer-token calls are added
for the `app` example later, they must validate Irongate access tokens locally before returning app
data.

## Token Storage

The BFF stores refresh-token state server-side in Cloudflare Durable Objects.

- Session IDs are random and opaque.
- Browser cookies contain no OAuth token material.
- Logout calls Irongate `/oauth/revoke` and clears the app session.
- Access tokens are short-lived and can be kept in BFF memory or refreshed on demand.
- Cloudflare KV is not used for sessions, refresh tokens, OAuth state, CSRF state, or logout state.

## Security Invariants

- Web example is BFF-only.
- No browser `localStorage` or `sessionStorage` token storage.
- No refresh token exposure to frontend JavaScript.
- No client secret in browser code.
- CSRF protection for state-changing BFF routes.
- OAuth `state` is validated.
- PKCE is used even if the BFF is confidential.
- Auth codes are removed from browser-visible URLs after callback handling.
- Protected API routes do not read Irongate DynamoDB tables.
- Third-party scripts and analytics are absent by default.
- Google and Apple sign-in are hidden or disabled until provider secrets and smoke tests exist.
- Generated `workers.dev` URLs are acceptable for first dev deploys; production should use an explicit domain.
- Irongate exact redirect URI matching remains the primary protection against unexpected origins.
- A Worker-side allowed-origin guard should be added before treating the web example as production-ready.

## Out Of Scope

- Direct SPA token storage as a recommended example.
- Separate shared protected API package.
- Hosted login UI inside Irongate core.
- Cookie-session support inside Irongate core.
- Google or Apple sign-in in the first web slice.
