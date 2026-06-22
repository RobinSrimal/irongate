# Web Example

Target code: `packages/examples/web`

## Owns

- Browser web application example.
- BFF session handling.
- OAuth callback handling for the web client.
- Server-side refresh-token storage.
- Password registration, email verification, login, callback, signed-in page, and logout.
- Google login handoff when Google provider config is enabled.
- Apple login handoff when Apple provider config and private-key secret are enabled.

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
3. BFF starts Irongate `/authorize` with `provider=password` for password login.
4. BFF captures the Irongate password authorize-session key and renders its own password form.
5. BFF posts the password credentials to Irongate `/password/login`.
6. Irongate redirects back to the BFF callback with an authorization code.
7. BFF validates state and exchanges the code at Irongate `/token`.
8. BFF stores the refresh token server-side under an opaque application session ID.
9. BFF sets the browser session cookie.
10. BFF renders a minimal signed-in page.

For Google login, the BFF uses the same callback and session handling but starts Irongate with:

```text
provider=google
```

The BFF never exchanges codes directly with Google. Irongate owns Google provider state, Google code
exchange, Google ID-token validation, identity mapping, and internal authorization-code issuance.

The Google login button is shown only when the deployed stage enables Google provider config. The
Google client secret stays in SST secrets. The checked-in stage config may contain the non-secret
Google client ID.

Apple login uses the same BFF callback and session model. The BFF starts Irongate with:

```text
provider=apple
```

Irongate owns Apple `form_post` callback handling, Apple client-secret JWT generation, Apple token
exchange, Apple ID-token validation, identity mapping, and internal authorization-code issuance.

The Apple login link is shown only when the deployed stage explicitly enables Apple provider config.
Apple non-secret identifiers may live in stage config, but the `.p8` private key stays in SST
secrets.

## Application Routes

The web example is focused on auth integration: login, callback handling, session creation, signed-in
state, and logout.

Browser-facing application routes validate BFF session state. Direct bearer-token routes validate
Irongate access tokens locally before returning application data.

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
- Google sign-in is shown only when provider config is enabled for the stage.
- Apple sign-in is shown only when provider config and the private-key secret are enabled for the
  stage.
- Generated `workers.dev` URLs are acceptable for first dev deploys; production should use an explicit domain.
- Irongate exact redirect URI matching remains the primary protection against unexpected origins.
- A Worker-side allowed-origin guard should be added before treating the web example as production-ready.

## Boundaries

- Browser OAuth tokens stay out of JavaScript-accessible storage.
- Shared application APIs live in the application layer, not in Irongate core.
- Hosted login UI lives in the web example, not inside the auth Lambda.
- Cookie-session support belongs to the BFF, not Irongate core.
