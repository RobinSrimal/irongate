# 33_web_apple_oidc_login_smoke

## Goal

Add Sign in with Apple to the optional Cloudflare web example and validate the full browser redirect
flow against the deployed Irongate dev API.

At the end of this slice, the deployed web example should support password, Google, and Apple login:

```text
browser
  -> Cloudflare Worker web BFF
  -> Irongate /authorize?provider=apple
  -> Apple consent/login
  -> Irongate /apple/callback
  -> Worker /auth/callback
  -> Worker session cookie
```

This slice is a deployed smoke-validation slice. It should prove the existing Apple OIDC core works
in a real browser environment before any Security Lab or richer signed-in app surface is added.

## Design Docs Followed

This slice follows and updates these design documents:

- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/api/providers/apple.md`
- `design/auth/providers/apple.md`
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

The important design constraint is that Irongate core remains API-only. The browser-facing Apple
link belongs to the optional web example, not the auth Lambda.

## Why This Slice Next

Google login now works through the deployed web BFF. Apple is the remaining first-class external
provider in the target core.

Testing Apple from the web example validates:

- Apple provider configuration
- Apple-generated `form_post` callback into Irongate
- Apple client-secret JWT generation
- Irongate provider-state storage
- Irongate Apple callback handling
- Apple issuer-plus-sub identity mapping
- internal authorization-code issuance
- Worker token exchange
- Worker session creation
- `/userinfo` after Apple login

Apple has more setup than Google, so it stays isolated from the Security Lab and other example-app
work.

## Scope Decision

In scope:

- Add dev-stage Apple provider config wiring in SST infra.
- Store the Apple private key in an SST secret, not in checked-in config.
- Add an Apple login start link to `packages/examples/web`.
- Reuse the existing Worker OAuth callback and session-creation path.
- Keep password and Google login working.
- Deploy dev after Apple configuration is set.
- Complete a browser Apple login smoke test.
- Validate DynamoDB provider-state, identity, authorization-code, and refresh-token key shapes with
  bounded queries or exact known keys where possible.
- Update design docs if implementation details change the documented web/provider setup.

Out of scope:

- Security Lab.
- Google changes beyond keeping the existing flow working.
- Apple login in the future Tauri app.
- Hosted Irongate UI.
- Direct browser token storage.
- A generic provider registry.
- Auto-linking Apple identities to password or Google accounts by email.
- Adding Apple buttons to Irongate core.
- Making the Worker session backend production-complete beyond the planned Durable Object direction.

## Configuration Model

Apple needs several non-secret identifiers and one private-key secret.

Checked-in dev stage config should hold the non-secret identifiers already created in Apple:

```text
auth.apple.enabled=false
auth.apple.clientId="com.auth.irongate"
auth.apple.teamId="XUTMJDN8V6"
auth.apple.keyId="W4DMH8K6X2"
auth.apple.clientSecretTtlSeconds optional
```

`enabled=false` keeps dev deploys safe until Apple Developer Support resolves private-key access and
the `.p8` file can be added as an SST secret.

SST secrets should hold:

```text
ApplePrivateKey -> AUTH_APPLE_PRIVATE_KEY
```

The auth Lambda should receive:

```text
AUTH_APPLE_CLIENT_ID=<stageConfig.auth.apple.clientId>
AUTH_APPLE_TEAM_ID=<stageConfig.auth.apple.teamId>
AUTH_APPLE_KEY_ID=<stageConfig.auth.apple.keyId>
AUTH_APPLE_PRIVATE_KEY_SECRET=AUTH_APPLE_PRIVATE_KEY
AUTH_APPLE_PRIVATE_KEY=<ApplePrivateKey SST secret value>
AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS=<optional stage value>
```

The secret should be stage-specific:

```text
sst secret set ApplePrivateKey <apple-private-key-pem> --stage dev
```

If Apple is not enabled for a stage, Apple login should be disabled in that stage and the web example
should not render a usable Apple login action.

Apple must be configured with this Irongate callback URL:

```text
https://1e88qilxk6.execute-api.eu-west-1.amazonaws.com/apple/callback
```

For production, this must be the production Irongate issuer domain:

```text
https://<production-auth-domain>/apple/callback
```

The Worker callback remains the OAuth client redirect URI:

```text
https://irongate-dev-examplewebworkerscript.robin-srimal.workers.dev/auth/callback
```

That URI stays in `auth.clients.toml` as the Irongate OAuth client callback.

## Web Example Behavior

Add a browser-facing Apple login entry point to the Worker:

```text
GET /auth/login
  renders password login form
  renders Google login option only when enabled
  renders Apple login option only when enabled

GET /auth/login/apple
  creates PKCE + state transaction cookie
  redirects to Irongate /authorize with provider=apple

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
provider=apple
```

The Worker must not exchange directly with Apple. Apple code exchange, Apple client-secret JWT
generation, and ID-token validation stay inside Irongate.

Apple returns to Irongate using `response_mode=form_post`, so the Irongate callback is:

```text
POST /apple/callback
```

The Worker still receives the final Irongate internal OAuth redirect as:

```text
GET /auth/callback?code=...&state=...
```

## Security Invariants

- Apple private key is never committed, logged, or sent to the Worker browser response.
- Apple client-secret JWTs are generated by Irongate at runtime and are not stored.
- Browser JavaScript never receives Irongate refresh tokens.
- Worker keeps the same HttpOnly Secure SameSite browser-session cookie model.
- Worker OAuth `state` is generated and validated for Apple just like password and Google login.
- Worker uses PKCE for Apple just like password and Google login.
- Irongate provider state is HMAC-keyed and single-use.
- Irongate identity key is Apple issuer plus Apple `sub`, not email.
- Apple identities are not auto-linked to password or Google users by email.
- Apple email/name claims may be missing and must not be required by the web example.
- Disabled or deleted accounts cannot complete Apple login.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add infra/stage config fields for optional Apple provider enablement.
2. Add `ApplePrivateKey` as an SST secret and wire it only into the public auth Lambda when Apple is
   fully configured.
3. Add static infra validation for Apple secret/config/env wiring.
4. Update `.example.env` and secret docs to explain that Apple private-key material lives in SST
   secrets.
5. Add web example config for `appleLoginEnabled`.
6. Add web route tests proving Apple login is hidden when disabled and rendered when enabled.
7. Add a route test proving `GET /auth/login/apple` redirects to Irongate `/authorize` with
   `provider=apple`, PKCE, state, nonce, and the Worker callback URI.
8. Implement the Apple login route and view changes.
9. Run local checks.
10. Configure Apple Developer portal callback and domain/service settings.
11. Set the dev Apple private-key SST secret.
12. Deploy dev.
13. Complete the Apple login in a browser.
14. Validate `/app` shows a signed-in user after Apple login even if Apple does not return email on
    later logins.
15. Validate logout still clears the Worker session and revokes Irongate refresh state.
16. Validate DynamoDB key shape for `provider:state`, `identity:apple`, `oauth:code`, and
    `oauth:refresh` records without using table scans.
17. Record any live smoke findings in a short note or in the slice result.

## Tests

Add or update focused tests under `packages/examples/web/tests`.

Required tests:

- Login page hides Apple when `appleLoginEnabled=false`.
- Login page shows Apple when `appleLoginEnabled=true`.
- `GET /auth/login/apple` creates an OAuth transaction cookie.
- `GET /auth/login/apple` redirects to Irongate `/authorize`.
- The Apple authorize URL contains `provider=apple`.
- The Apple authorize URL contains a PKCE challenge and `code_challenge_method=S256`.
- The Apple authorize URL uses the Worker `/auth/callback` as `redirect_uri`.
- Existing password and Google login tests still pass.

Add infra validation tests or static checks proving:

- `ApplePrivateKey` SST secret exists.
- The public auth Lambda receives Apple environment only when Apple stage config is complete.
- The Worker receives only a non-secret Apple enabled flag.

Rust provider tests already cover Apple OIDC internals. Add Rust tests only if this slice changes
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

Apple Developer setup:

```text
Services ID / client ID: <stageConfig.auth.apple.clientId>
Return URL:
https://1e88qilxk6.execute-api.eu-west-1.amazonaws.com/apple/callback
```

SST secret setup:

```text
sst secret set ApplePrivateKey <apple-private-key-pem> --stage dev
```

Deploy:

```text
npm run deploy -- --stage dev
```

Browser smoke:

```text
open https://irongate-dev-examplewebworkerscript.robin-srimal.workers.dev/auth/login
click Continue with Apple
complete Sign in with Apple
confirm redirect to /app
confirm logout works
```

DynamoDB validation should use bounded queries, for example:

```text
aws dynamodb query \
  --table-name irongate-dev-AuthTableTable-wzwedmtx \
  --key-condition-expression "pk = :pk" \
  --expression-attribute-values '{":pk":{"S":"identity:apple"}}'
```

Expected:

- Raw Apple provider state is not present in `pk` or `sk`.
- Raw Irongate authorization code is not present in `pk` or `sk`.
- Apple identity records use `identity:apple` with a digest key.
- The identity record subject maps to an active account.
- Login creates a Worker session and does not expose refresh tokens to browser JavaScript.

## Acceptance Criteria

- Apple provider config is wired through stage config and SST secrets.
- The web example shows Apple login only when enabled.
- Apple login starts from the deployed Worker in a browser.
- Irongate handles Apple callback and returns an internal authorization code to the Worker callback.
- The Worker exchanges the Irongate code and creates its normal session cookie.
- `/app` works after Apple login.
- Logout still works after Apple login.
- Password and Google login still work.
- No Apple private key or client-secret JWT appears in committed files, logs, browser HTML, or
  browser JavaScript.
- Provider state, authorization codes, and refresh tokens are not stored as raw DynamoDB keys.
- No Security Lab or app-example work is included.

## Next Slice

After this slice, define:

```text
34_web_security_lab_foundation
```

That slice can make the signed-in web example more useful once password, Google, and Apple all work
through the BFF.
