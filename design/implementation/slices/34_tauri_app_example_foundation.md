# 34_tauri_app_example_foundation

## Goal

Create the first optional native app example as a desktop-first Tauri app using React and
TypeScript, then wire it to the same deployed Irongate auth backend already used by the web
example.

At the end of this slice, `packages/examples/app` should be a minimal native reference client that
can sign in, hold a local session, refresh, and log out without depending on Cloudflare.

## Design Docs Followed

This slice follows and updates:

- `design/examples/README.md`
- `design/examples/app.md`
- `design/examples/client-profiles.md`
- `design/auth/config/clients.md`
- `design/auth/config/client-file.md`
- `design/auth/core/clients.md`
- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/core/tokens.md`
- `design/auth/crypto/cookies.md`
- `design/infra/examples/README.md`
- `design/implementation/ROADMAP.md`

The important design constraint is that the app example is a local/native client. It must not add
Cloudflare infrastructure, a web BFF dependency, a docs site, a Security Lab, or new Irongate core
runtime behavior.

## Why This Slice Next

The web example now proves the browser BFF path with password, Google, and Apple. The next useful
best-practice example is the native app path:

```text
Tauri app
  -> Irongate on AWS
  -> loopback callback
  -> OS keychain refresh-token storage
```

This keeps this repo lean while proving that Irongate can be used by a native desktop client without
reusing Cloudflare Worker sessions or browser cookie assumptions.

## Scope Decision

This is an app-foundation slice.

In scope:

- Preserve the human-created `packages/examples/app` scaffold from the official Tauri wizard.
- Use React and TypeScript for the app frontend.
- Keep the wizard-generated Tauri project shape where practical.
- Add root/workspace scripts for building and testing the app example.
- Add a native desktop OAuth client entry to `auth.clients.toml`.
- Use a loopback redirect URI with dynamic runtime port support already implemented in auth core.
- Add app config for issuer URL, client ID, scopes, and loopback callback path.
- Implement PKCE and state generation in the app.
- Implement a loopback callback listener in the Tauri Rust side.
- Implement sign in with Google and Apple through the external system browser.
- Implement password login for an existing verified password account using the app React form and
  Irongate password APIs.
- Exchange authorization codes through Irongate `/token`.
- Store refresh tokens in OS keychain or credential manager behind a narrow abstraction.
- Keep access tokens in memory only.
- Implement refresh and logout using Irongate `/token` and `/oauth/revoke`.
- Add a minimal signed-in screen showing sanitized userinfo and provider/source claims.
- Add focused tests for PKCE/state helpers and token/session state where feasible without launching
  a full desktop app.
- Update docs with local run instructions and the required Irongate client config.

Out of scope:

- Running the Tauri wizard from the agent.
- Mobile implementation.
- Tauri Stronghold.
- Cloudflare Worker changes.
- Cloudflare infrastructure.
- Security Lab.
- Docs website.
- Protected resource API.
- New Irongate core auth features.
- Hosted auth UI in Irongate core.
- Generic provider support beyond password, Google, and Apple.
- App packaging, signing, notarization, installers, or app-store distribution.

## Scaffold State

The human operator created the initial app with the current Tauri wizard:

```text
npm create tauri-app@latest
```

Choose:

```text
project directory: packages/examples/app
frontend: React
language: TypeScript
package manager: npm
```

The official Tauri quick-start documents `npm create tauri-app@latest` as a current project creation
path. The implementation should preserve this scaffold and remove only wizard-demo behavior as auth
features replace it.

## Target App Tree

The exact generated files may vary by Tauri version, but the target shape should be close to:

```text
packages/examples/app/
  package.json
  tsconfig.json
  tsconfig.node.json
  vite.config.ts
  index.html
  src/
    App.tsx
    main.tsx
    auth/
      oauth.ts
      pkce.ts
      session.ts
      tokenStore.ts
  src-tauri/
    Cargo.toml
    tauri.conf.json
    capabilities/
      default.json
    src/
      lib.rs
      main.rs
      auth.rs
  tests/
```

Keep files small enough to review. For the first pass, the Rust auth command module may own browser
launch, loopback callback handling, token exchange, refresh, revoke, and keychain storage together.
Split it later if platform-specific keychain behavior or mobile support makes that module hard to
reason about.

## Auth Flow

### Provider Login

Google and Apple login should use the external system browser:

```text
app creates PKCE + state
app starts loopback listener
app opens system browser to /authorize?provider=google|apple
Irongate handles provider callback
Irongate redirects to http://127.0.0.1:<port>/oauth/callback?code=...&state=...
app validates state
app exchanges code at /token
app stores refresh token in OS keychain
app keeps access token in memory
```

### Password Login

Because Irongate core is API-only and has no hosted login UI, the app owns its password form.

Target flow:

```text
app creates PKCE + state
app calls /authorize with client_id=app and redirect_uri=http://127.0.0.1:<port>/oauth/callback
app captures the authorize session from the 303 Location
app posts email, password, and session to /password/login
Irongate redirects to loopback callback with code and state
app validates state
app exchanges code at /token
```

Password registration, email verification, and password reset can remain web-assisted or deferred in
this first app slice. The first app slice should support login for an existing verified password
account.

### Refresh And Logout

Refresh:

```text
read refresh token from keychain
POST /token grant_type=refresh_token
replace refresh token in keychain when rotated
keep new access token in memory
```

Logout:

```text
read refresh token from keychain
POST /oauth/revoke
delete refresh token from keychain
clear access token and userinfo from memory
```

## Client Configuration

Add a native desktop client to `auth.clients.toml`:

```toml
[[clients]]
client_id = "app"
client_type = "native_desktop"
redirect_uris = ["http://127.0.0.1/oauth/callback"]
allowed_grant_types = ["authorization_code", "refresh_token"]
allowed_scopes = ["openid", "profile", "email", "offline_access"]
pkce_required = true
token_endpoint_auth_method = "none"
```

The auth core already supports dynamic loopback ports for `native_desktop`. The app must register a
runtime port only on loopback hosts and keep the path `/oauth/callback`.

## Security Invariants

- Native app is a public OAuth client.
- No client secret is stored in the app.
- PKCE is required for every authorization-code flow.
- Google and Apple login use the external system browser.
- No embedded WebView provider login.
- Password login uses the app-owned React form because Irongate core is API-only.
- Refresh tokens are stored only through OS-backed keychain or credential storage.
- Access tokens stay in memory only.
- Logout revokes the refresh-token family and clears local storage.
- The app does not read Irongate DynamoDB tables.
- The app does not depend on Cloudflare Worker sessions.
- The app does not add frontend hosting infrastructure.

## Expected Code Shape

Implementation should preserve the wizard scaffold and then add thin modules:

```text
src/auth/pkce.ts              browser-safe PKCE and state helpers
src/auth/oauth.ts             authorize URL and password-session helpers
src/auth/session.ts           in-memory session state
src/auth/tokenStore.ts        frontend wrapper over Tauri keychain commands
src-tauri/src/auth.rs         open-browser, loopback, token, revoke, userinfo, and keychain commands
```

The first implementation keeps the Rust command surface in one module to avoid premature structure.
The public command interface should stay narrow even if the internals are split later.

## Tests

Local tests should cover:

- PKCE verifier and challenge format.
- State generation and validation.
- Authorize URL construction for Google and Apple.
- Password login authorize-session parsing.
- Token response parsing.
- Frontend keychain command wrappers where feasible without launching a full desktop app.
- Logout clears in-memory state after revoke succeeds or after an idempotent revoke response.

Manual validation should cover:

- App launches locally.
- Google login reaches Irongate and returns to loopback.
- Apple login reaches Irongate and returns to loopback.
- Password login works for an existing verified account.
- Refresh works after app restart using the keychain-stored refresh token.
- Logout revokes and removes the stored refresh token.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Human operator creates the wizard scaffold under `packages/examples/app`.
2. Add app package metadata, workspace scripts, and test script alignment.
3. Add the `app` native desktop client to `auth.clients.toml`.
4. Add app config and environment documentation.
5. Add PKCE/state helpers and tests.
6. Add authorize URL builders and tests for Google and Apple.
7. Add loopback listener command in Tauri Rust.
8. Add system-browser opener command.
9. Add token exchange, refresh, revoke, and userinfo helpers.
10. Add keychain-backed refresh-token storage through the Rust command layer.
11. Add login UI with password, Google, and Apple actions.
12. Add signed-in UI and logout action.
13. Run app local smoke test against deployed Irongate dev.
14. Update design/docs with any scaffold-specific path differences.
15. Run final verification.

## Acceptance Criteria

- `packages/examples/app` exists as a Tauri React TypeScript app generated from the wizard.
- The app has no Cloudflare dependency.
- `auth.clients.toml` includes a native desktop `app` client.
- Google and Apple use the external system browser.
- Password login works for an existing verified account.
- The app exchanges authorization codes directly with Irongate.
- Refresh token storage goes through OS keychain or credential storage abstraction.
- Access tokens are not persisted.
- Logout revokes refresh-token state and clears local storage.
- App tests cover PKCE/state and auth helper behavior.
- Docs state that docs/lab will be built in a separate downstream repo, not this template.
- No Irongate core runtime behavior changes are required unless implementation exposes a concrete
  bug.

## Manual Validation

Expected local commands after implementation:

```text
npm --workspace @irongate/example-app test
npm --workspace @irongate/example-app run tauri dev
```

Expected auth smoke:

```text
Google sign in -> loopback callback -> signed-in screen
Apple sign in -> loopback callback -> signed-in screen
Password sign in with verified account -> signed-in screen
Quit app -> reopen -> refresh from keychain succeeds
Logout -> refresh token revoked and local keychain entry removed
```

## Next Slice

After this slice, define the next slice based on the app smoke results.

Likely follow-ups:

- Add password registration and reset UX to the app.
- Add app mobile notes once desktop behavior is stable.
- Tighten keychain behavior per platform.
- Keep docs and Security Lab in a separate downstream repo created from this template.
