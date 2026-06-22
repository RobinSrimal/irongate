# 32_web_google_oidc_login_smoke

## Goal

Add Google login to the optional Cloudflare web example and validate the full browser redirect flow
against the deployed Irongate dev API.

At the end of this slice, the deployed web example should support both password login and Google
login:

```text
browser
  -> Cloudflare Worker web BFF
  -> Irongate /authorize?provider=google
  -> Google consent/login
  -> Irongate /google/callback
  -> Worker /auth/callback
  -> Worker session cookie
```

This slice is primarily a deployed smoke-validation slice. It should prove the existing Google OIDC
core works in a real browser environment before Apple or richer example-app features are added.

## Design Docs Followed

This slice follows and updates these design documents:

- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/api/providers/google.md`
- `design/auth/providers/google.md`
- `design/auth/config/environment.md`
- `design/auth/store/provider-states.md`
- `design/auth/store/authorization-codes.md`
- `design/auth/store/identities.md`
- `design/auth/store/keys.md`
- `design/examples/web.md`
- `design/infra/auth/secrets.md`
- `design/infra/examples/web.md`
- `design/infra/shared/stages.md`
- `design/implementation/ROADMAP.md`

The important design constraint is that Irongate core remains API-only. The browser-facing Google
button belongs to the optional web example, not the auth Lambda.

## Why This Slice Next

The password-based deployed web flow works. Google OIDC is already implemented in the auth core, but
it has not yet been validated through the deployed browser path.

Testing Google from the web example validates more than a raw `/google/authorize` curl smoke test:

- browser redirects
- Google provider configuration
- Irongate provider-state storage
- Irongate callback handling
- internal authorization-code issuance
- Worker token exchange
- Worker session creation
- `/userinfo` after external-provider login

Apple should remain separate because Apple setup has more moving parts and should not obscure the
Google smoke result.

## Scope Decision

In scope:

- Add dev-stage Google provider config wiring in SST infra.
- Store the Google client secret in an SST secret, not in checked-in config.
- Add a Google login start route/button to `packages/examples/web`.
- Reuse the existing Worker OAuth callback and session-creation path.
- Keep the existing password login flow working.
- Deploy dev after configuration is set.
- Complete a browser Google login smoke test.
- Validate DynamoDB provider-state, identity, authorization-code, and session key shapes with bounded
  queries or exact known keys where possible.
- Update design docs if implementation details change the documented web/provider setup.

Out of scope:

- Apple login.
- Google sign-in in the future Tauri app.
- Hosted Irongate UI.
- Direct browser token storage.
- A generic provider registry.
- Auto-linking Google identities to password accounts by email.
- Adding Google buttons to Irongate core.
- Building the Security Lab.
- Changing the resource model for the web example.
- Making the Worker session backend production-complete beyond the already planned Durable Object
  direction.

## Configuration Model

Google needs one non-secret value and one secret value.

Checked-in stage config should hold:

```text
AUTH_GOOGLE_CLIENT_ID=<Google OAuth web client ID>
```

SST secrets should hold:

```text
GoogleClientSecret -> AUTH_GOOGLE_CLIENT_SECRET
```

The secret should be stage-specific:

```text
sst secret set GoogleClientSecret <google-client-secret> --stage dev
```

If the Google client ID is not configured for a stage, Google login should be disabled in that stage
and the web example should not render a usable Google login action.

Google must be configured with this Irongate redirect URI:

```text
https://1e88qilxk6.execute-api.eu-west-1.amazonaws.com/google/callback
```

For production, this must be the production Irongate issuer domain:

```text
https://<production-auth-domain>/google/callback
```

The Worker callback remains the OAuth client redirect URI:

```text
https://irongate-dev-examplewebworkerscript.robin-srimal.workers.dev/auth/callback
```

That URI stays in `auth.clients.toml` as the Irongate OAuth client callback.

## Web Example Behavior

Add a browser-facing Google login entry point to the Worker:

```text
GET /auth/login
  renders password login form
  renders Google login option only when enabled

GET /auth/login/google
  creates PKCE + state transaction cookie
  redirects to Irongate /authorize with provider=google

GET /auth/callback
  unchanged shared OAuth callback
  exchanges Irongate authorization code for tokens
  calls /userinfo
  stores server-side session
  redirects to /app
```

Suggested authorize parameters:

```text
response_type=code
client_id=<web client id>
redirect_uri=<Worker /auth/callback>
scope=openid email profile offline_access
state=<Worker-generated state>
nonce=<Worker-generated nonce>
code_challenge=<S256 challenge>
code_challenge_method=S256
provider=google
```

The Worker must not exchange directly with Google. Google code exchange and ID-token validation stay
inside Irongate.

## Security Invariants

- Google client secret is never committed, logged, or sent to the Worker browser response.
- Browser JavaScript never receives Irongate refresh tokens.
- Worker keeps the same HttpOnly Secure SameSite browser-session cookie model.
- Worker OAuth `state` is generated and validated for Google just like password login.
- Worker uses PKCE for Google just like password login.
- Irongate provider state is HMAC-keyed and single-use.
- Irongate identity key is Google issuer plus Google `sub`, not email.
- Google identities are not auto-linked to password users by email.
- Disabled or deleted accounts cannot complete Google login.
- The deployed web example should not show Apple as available in this slice.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add infra/stage config fields for optional Google provider enablement.
2. Add `GoogleClientSecret` as an SST secret and wire it only into the public auth Lambda when Google
   is configured.
3. Update `.example.env` and secret docs to explain that Google secrets live in SST secrets.
4. Add web example config for `googleLoginEnabled`.
5. Add web route tests proving Google login is hidden when disabled and rendered when enabled.
6. Add a route test proving `GET /auth/login/google` redirects to Irongate `/authorize` with
   `provider=google`, PKCE, state, nonce, and the Worker callback URI.
7. Implement the Google login route and view changes.
8. Run local checks.
9. Set the dev Google SST secret.
10. Deploy dev.
11. Complete the Google login in a browser.
12. Validate `/app` shows a signed-in user after Google login.
13. Validate logout still clears the Worker session and revokes Irongate refresh state.
14. Validate DynamoDB key shape for `provider:state`, `identity:google`, `oauth:code`, and
    `oauth:refresh` records without using table scans.
15. Record any live smoke findings in a short note or in the slice result.

## Tests

Add or update focused tests under `packages/examples/web/tests`.

Required tests:

- Login page hides Google when `googleLoginEnabled=false`.
- Login page shows Google when `googleLoginEnabled=true`.
- `GET /auth/login/google` creates an OAuth transaction cookie.
- `GET /auth/login/google` redirects to Irongate `/authorize`.
- The Google authorize URL contains `provider=google`.
- The Google authorize URL contains a PKCE challenge and `code_challenge_method=S256`.
- The Google authorize URL uses the Worker `/auth/callback` as `redirect_uri`.
- Existing password login tests still pass.

Rust provider tests already cover Google OIDC internals. Add Rust tests only if this slice changes
auth-core provider behavior.

## Manual Validation

Before deploy:

```text
npm run typecheck
npm run test:infra
npm run test:examples
cargo test --manifest-path packages/functions/auth/Cargo.toml
git diff --check
```

Google Cloud Console setup:

```text
Authorized redirect URI:
https://1e88qilxk6.execute-api.eu-west-1.amazonaws.com/google/callback
```

SST secret setup:

```text
sst secret set GoogleClientSecret <google-client-secret> --stage dev
```

Deploy:

```text
npm run deploy -- --stage dev
```

Browser smoke:

```text
open https://irongate-dev-examplewebworkerscript.robin-srimal.workers.dev/auth/login
click Google login
complete Google sign-in
confirm redirect to /app
confirm logout works
```

DynamoDB validation should use bounded queries, for example:

```text
aws dynamodb query \
  --table-name irongate-dev-AuthTableTable-wzwedmtx \
  --key-condition-expression "pk = :pk" \
  --expression-attribute-values '{":pk":{"S":"identity:google"}}'
```

Expected:

- Raw Google provider state is not present in `pk` or `sk`.
- Raw Irongate authorization code is not present in `pk` or `sk`.
- Google identity records use `identity:google` with a digest key.
- The identity record subject maps to an active account.
- Login creates a Worker session and does not expose refresh tokens to browser JavaScript.

## Acceptance Criteria

- Google provider config is wired through stage config and SST secrets.
- The web example shows Google login only when enabled.
- Google login starts from the deployed Worker in a browser.
- Irongate handles Google callback and returns an internal authorization code to the Worker callback.
- The Worker exchanges the Irongate code and creates its normal session cookie.
- `/app` works after Google login.
- Logout still works after Google login.
- Password login still works.
- No Google client secret appears in committed files, logs, browser HTML, or browser JavaScript.
- Provider state, authorization codes, and refresh tokens are not stored as raw DynamoDB keys.
- No Apple UI or Apple config changes are included.

## Next Slice

After this slice, define one of:

```text
33_web_apple_oidc_login_smoke
```

Apple is next so all first-class external providers are validated through the browser BFF before the
Security Lab is added.
