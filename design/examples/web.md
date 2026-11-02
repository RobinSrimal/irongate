# Web Example

Implementation: [Worker](web/src/worker.md), [routes](web/src/routes.md),
[session storage](web/src/session.md) and [OAuth helpers](web/src/oauth.md).

## What And Why

A self-contained Cloudflare Worker BFF provides password registration, verification, login, optional
Google/Apple login, callback handling, a signed-in page and logout. It keeps OAuth tokens outside
browser JavaScript while the AWS auth Lambda remains API-only.

The configured OAuth client is public (`spa`, `none` authentication), not confidential.
There is no client-secret binding or authenticated token-exchange implementation in this Worker.

## Flow

1. Create a random login identifier, state, nonce and PKCE verifier.
2. Store the login record in a Durable Object and set an HttpOnly login cookie.
3. For password login, obtain an Irongate authorize session and forward the submitted credentials.
   For Google/Apple, redirect through Irongate's provider endpoints using the external browser.
4. Validate callback state, exchange the code with PKCE, and fetch userinfo.
5. Store tokens and userinfo under an opaque application-session identifier; redirect to `/app`.
6. On logout, attempt refresh-token revocation, delete session state and clear the cookie.

Cookies use HttpOnly and SameSite=Lax, with Secure on HTTPS. Views escape content and set
security headers. Callback handling redirects away from the code-bearing URL.

## Deployment

The Worker derives its base URL from the request origin unless `WEB_BASE_URL` is set.
Irongate independently requires the exact callback in `auth.clients.toml`.
The local Node adapter needs an explicit base URL. Cloudflare deployment, domains and the Durable
Object binding are described in [web infrastructure](../infra/examples/web.md).

## Current Limits

- Login/session records contain expiry timestamps, but storage and route reads do not enforce them.
  Browser cookie age is not server-side expiry enforcement.
- Durable Objects provide record storage; the complete read/exchange/delete workflow is not one
  serialized transaction. The Worker does not refresh access tokens or rotate refresh tokens.
- The signed-in page displays cached userinfo; it does not revalidate account status on every request.
- OAuth state is checked, but the generated OIDC nonce is not independently validated against the ID
  token in the Worker.
- State-changing routes have no explicit CSRF-token or origin-allowlist check.
- Verification GET forwards the token to Irongate's consuming POST endpoint, so an email scanner
  opening the link can consume it.
- There is no password-reset page/handler and no separate bearer-protected resource API.

These are limits of the current example, not guarantees supplied by the BFF pattern or Durable
Objects. [Route tests](web/tests/routes.md) use fake upstream responses and do not certify deployed
Cloudflare behavior.
