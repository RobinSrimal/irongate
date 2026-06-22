# 31_cloudflare_web_example_foundation

## Goal

Define and then implement the first optional web example as a local-first Cloudflare Worker BFF,
then deploy it once the password-auth flow works locally.

At the end of this slice, the repo should have the smallest useful
`packages/examples/web` app for normal password auth against Irongate, verified locally first and
then smoke-tested as a deployed Worker:

```text
browser
  -> Cloudflare Worker web BFF
  -> Irongate auth API on AWS
  -> password registration / email verification / login
  -> browser session cookie
```

The web example should be self-contained. Do not introduce a separate shared protected API package yet.
The Security Lab and richer protected app experience come later.

## Design Docs Followed

This slice follows and updates:

- `design/examples/README.md`
- `design/examples/web.md`
- `design/examples/app.md`
- `design/infra/examples/README.md`
- `design/implementation/ROADMAP.md`

## Scope Decision

This slice focuses on a local-first web password-auth path, with deployment validation before the
slice is complete.

In scope:

- Create `packages/examples/web`.
- Use TypeScript for the Cloudflare Worker example.
- Make the Worker run locally against a deployed Irongate dev API.
- Add SST Cloudflare example infrastructure behind explicit example enablement after the local flow works.
- Deploy the web example to Cloudflare when credentials are available and the local flow passes.
- Define the Worker route surface for:
  - health check
  - password registration page/action
  - email verification link handling
  - password login start/form
  - OAuth callback
  - minimal signed-in page
  - logout
- Exercise the normal Irongate password flow end to end.
- Use Authorization Code + PKCE through Irongate.
- Store the browser session with an HttpOnly Secure SameSite cookie.
- Use Durable Objects as the planned authoritative session and refresh-token backend.
- Derive the web base URL from the incoming request origin by default.
- Keep `WEB_BASE_URL` / `examples.web.baseUrl` as an optional override for custom domains or local tunneling.
- Add package scripts and focused unit tests where feasible without requiring a live Cloudflare account.
- Add local-run notes and deployment notes for required Cloudflare and Irongate configuration.

Out of scope:

- Creating a separate shared protected API package.
- Implementing the Tauri app.
- Building the Security Lab.
- Building protected app API routes beyond a minimal signed-in page.
- Calling the web example from the future `app` example.
- Google or Apple sign-in.
- Adding D1, Durable Object, or secret bindings unless required for compile-time shape.
- Adding Cloudflare KV. KV is not part of the planned auth/session architecture.
- Changing Irongate core runtime behavior.

## Architecture Decision

For now, the web example owns only the browser password-auth integration:

```text
BFF auth/session routes
minimal signed-in page
```

The Worker is the deployed Cloudflare resource. Irongate remains the AWS-hosted authorization server.

Initial target shape:

```text
packages/examples/web/
  package.json
  tsconfig.json
  src/
    worker.ts
    config.ts
    oauth.ts
    session.ts
    routes.ts
    views.ts
  tests/
```

The first version does not need business data. It only needs to prove the normal
password-auth shape:

```text
register -> verify email -> login -> callback -> session cookie -> signed-in page
```

Implementation order is intentionally local-first:

```text
local Worker
  -> deployed Irongate dev API
  -> local password-auth smoke test
  -> Cloudflare deployment
  -> deployed Worker password-auth smoke test
```

The Worker should not require its deployed URL to be known before it starts. It derives its own
public base URL from `new URL(request.url).origin`, while allowing `WEB_BASE_URL` as an explicit
override. Irongate still needs the deployed callback URL in `auth.clients.toml`.

This request-origin fallback is for local development and first `workers.dev` deploys. Production
examples should use an explicit domain and exact Irongate client redirect URI. A follow-up hardening
slice should add a Worker-side allowed-origin guard so unexpected hosts fail before the Worker starts
OAuth.

The planned server-side session backend is Cloudflare Durable Objects:

```text
browser
  -> HttpOnly Secure session cookie
  -> Cloudflare Worker
  -> Durable Object session record
  -> Irongate refresh-token state held server-side
```

Durable Objects are the target because the BFF needs authoritative session state,
logout, and refresh-token rotation to be serialized consistently. Cloudflare KV is
not used for sessions, refresh tokens, CSRF state, OAuth state, logout state, or
any other auth authority.

## Security Invariants

- Browser receives only an HttpOnly Secure SameSite session cookie.
- Browser JavaScript does not receive refresh tokens.
- Worker stores refresh-token state server-side in Durable Objects.
- OAuth `state` is generated and validated.
- PKCE is used.
- Logout clears the browser cookie and will revoke refresh-token state once storage exists.
- No client secret is exposed to browser JavaScript.
- No Irongate DynamoDB access from the Worker.
- Cloudflare KV is not used for auth/session state.
- Google and Apple buttons are not shown as usable flows until provider secrets and smoke tests exist.
- Request-origin inference is acceptable for local/dev and first `workers.dev` deploys, but production should use an explicit domain.
- A future origin-allowlist guard should reject unexpected web origins before OAuth begins.

## Acceptance Criteria

- Slice docs no longer ask for a shared protected API package.
- Current example docs list only `web` and `app`.
- `packages/examples/web` exists with a minimal Cloudflare Worker TypeScript password-auth app.
- The local Worker can run against the deployed Irongate dev API.
- The local Worker can complete password registration, verification, login, callback, and logout.
- Example infra can deploy the web Worker only when examples are explicitly enabled.
- The deployed web app can run password registration, verification, login, callback, and logout against Irongate.
- The deployed Worker does not require `examples.web.baseUrl`; it can infer its origin from requests.
- Docs state that origin inference is for local/dev and first deploys, not the complete production hardening story.
- The Worker code is testable locally without deploying to Cloudflare.
- Tests cover route dispatch and security headers/cookie shape where implemented.
- Docs state that Durable Objects are the session/refresh-token storage backend.
- Docs do not introduce Cloudflare KV for auth/session state.
- Docs state that Google and Apple sign-in are deferred.
- No Irongate core runtime behavior changes.

## Next Slice

After this slice, define:

```text
32_web_google_oidc_login_smoke
```

That slice can add Google login to the deployed web example and smoke-test the provider flow in a
browser. The Security Lab can follow once password and Google login both work through the BFF.
