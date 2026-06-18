# 27_example_application_architecture

## Goal

Define the optional example application architecture for web, mobile, and desktop clients before building any example code.

At the end of this slice, the repo should have design docs that explain how Irongate examples will demonstrate high-security integration patterns while keeping the auth core API-only and frontend-agnostic.

## Design Docs Followed

This slice should follow and update these design documents:

- `design/scope.md`
- `design/migration.md`
- `design/auth/config/clients.md`
- `design/auth/config/client-file.md`
- `design/auth/core/clients.md`
- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/core/tokens.md`
- `design/auth/crypto/cookies.md`
- `design/infra/README.md`
- `design/infra/shared/stages.md`
- `design/infra/auth/api.md`
- `design/implementation/ROADMAP.md`

The important design constraint is that examples are reference implementations and documentation. They must not make frontend hosting, hosted login UI, Cloudflare, mobile, desktop, or sample business APIs part of Irongate core.

## Why This Slice Next

Slice 26 created the infra boundary for optional examples:

```text
infra/auth
infra/shared
infra/examples
```

The next decision is the example product shape. The examples should show high-security integration patterns across web, mobile, and desktop, but without accidentally committing the core to one frontend framework or hosting platform.

This slice should define the architecture first so later implementation slices can stay small and intentional.

## Scope Decision

This is a design-only slice.

In scope:

- Define the target `packages/examples` tree.
- Define the optional hosted login surface example.
- Define the web SPA example.
- Define the native mobile example.
- Define the native desktop example.
- Define the sample protected resource API example.
- Define client profiles needed to support those examples.
- Define redirect URI rules for web, mobile, and desktop.
- Define token storage guidance by platform.
- Define CORS/origin guidance for browser-based examples.
- Define optional example infra and hosting boundaries.
- Update design docs and roadmap to capture these decisions.

Out of scope:

- Implementing any example app.
- Deploying frontend hosting.
- Adding Cloudflare, S3, CloudFront, or native build tooling.
- Adding client-profile runtime enforcement in Rust.
- Adding loopback redirect matching in code.
- Adding BFF, token mediator, or DPoP implementation.
- Changing current auth API behavior.
- Changing current deployed AWS resources.

## Target Example Tree

The proposed future example tree is:

```text
packages/examples/
  auth-web/
  web-spa/
  mobile/
  desktop/
  resource-api/
```

Each example has a different job:

| Example | Purpose |
| --- | --- |
| `auth-web` | Optional browser-hosted login, registration, verification, reset, and provider-selection surface. |
| `web-spa` | Static browser app using Authorization Code + PKCE against Irongate. |
| `mobile` | Native client using the external system browser, PKCE, app/claimed redirects, and OS secure storage. |
| `desktop` | Native client using the external system browser, PKCE, loopback redirect, and OS keychain storage. |
| `resource-api` | Minimal protected API that validates Irongate access JWTs with issuer, audience, expiry, signature, and scopes. |

This slice should decide the architecture and security rules only. The actual framework choices can remain deferred unless the design needs a placeholder.

## Hosted Login Surface

`auth-web` should be an optional example hosted login surface, not a core hosted UI.

It demonstrates this pattern:

```text
web app / mobile app / desktop app
  -> browser/system browser
  -> auth-web login surface
  -> Irongate API
  -> redirect back with authorization code
  -> PKCE token exchange
```

`auth-web` owns:

- login forms
- registration forms
- email verification token handling
- password reset token handling
- provider selection
- callback/error presentation

Irongate core still owns:

- OAuth/OIDC protocol behavior
- password verification and account state
- provider callback validation
- authorization-code issuance
- token issuance
- refresh rotation and logout
- DynamoDB storage
- audit and rate limits

Security rules for `auth-web`:

- No client secrets in browser code.
- No refresh token storage in browser storage by default.
- No analytics or third-party scripts by default.
- Strict Content Security Policy.
- No auth codes or tokens logged.
- Remove authorization codes from browser-visible URLs after handling where applicable.
- Verify and preserve `state`.
- Use `nonce` for OIDC requests that require ID tokens.

## Client Profiles

The example architecture should define these future client profiles:

```text
spa
native_mobile
native_desktop
web_confidential
```

Expected profile rules:

| Profile | Secret? | PKCE | CORS | Redirects |
| --- | --- | --- | --- | --- |
| `spa` | No | Required | Required for browser token calls | Exact HTTPS or localhost dev callback |
| `native_mobile` | No | Required | Not relevant | Claimed HTTPS/app links preferred, custom scheme allowed |
| `native_desktop` | No | Required | Not relevant | Loopback redirect with dynamic port |
| `web_confidential` | Yes | Recommended or required by policy | Usually no browser CORS to token endpoint | Exact HTTPS callback |

This slice should not implement the profiles. It should document the target behavior so a later implementation slice can add `client_type`, `allowed_origins`, and redirect matching rules deliberately.

## Redirect Rules

The design should define redirect rules for each example class:

### Web SPA

- Registered redirect URIs are exact.
- Production redirects are HTTPS.
- Local development may use localhost.
- Wildcard redirect URIs are not allowed.

### Mobile

- Claimed HTTPS redirects such as Universal Links or Android App Links are preferred.
- Private-use custom schemes may be supported for examples.
- PKCE is always required.
- The app must use the external/system browser, not an embedded WebView.

### Desktop

- Loopback redirect is allowed for native desktop clients.
- The registered redirect fixes scheme, host, and path.
- The runtime port may vary.
- Only loopback hosts are eligible for dynamic port matching.
- PKCE is always required.
- The app must use the external/system browser.

The biggest future Irongate code change from this design is likely dynamic-port loopback matching for `native_desktop`.

## Token Storage Guidance

The examples should demonstrate different storage expectations:

| Example | Token storage guidance |
| --- | --- |
| `auth-web` | Should not store access or refresh tokens; it is a login surface. |
| `web-spa` | Prefer in-memory access tokens; avoid localStorage; refresh tokens only with rotation and clear risk documentation. |
| `mobile` | Store refresh tokens in OS secure storage such as Keychain/Keystore. |
| `desktop` | Store refresh tokens in OS keychain/credential manager. |
| `resource-api` | Does not store user tokens; validates access JWTs per request. |

Docs should be explicit that no browser storage pattern fully protects against malicious JavaScript running in the app origin.

## Example Infra Boundary

Example deployment remains opt-in.

Future example infra may include:

- hosted `auth-web`
- hosted `web-spa`
- hosted `resource-api`
- custom domains for examples
- environment variables pointing examples at the deployed Irongate issuer

Default behavior remains:

```text
examples.enabled = false
deploy auth core only
```

Example infra must not be imported or deployed unless a stage enables it deliberately.

## Documentation Artifacts

Create or update:

```text
design/examples/README.md
design/examples/auth-web.md
design/examples/web-spa.md
design/examples/mobile.md
design/examples/desktop.md
design/examples/resource-api.md
design/examples/client-profiles.md
```

Update existing docs:

```text
design/scope.md
design/migration.md
design/auth/config/clients.md
design/auth/config/client-file.md
design/auth/core/clients.md
design/infra/README.md
design/infra/shared/stages.md
design/implementation/ROADMAP.md
```

If a listed existing doc already has equivalent guidance, update it instead of duplicating content.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Create `design/examples/README.md` with the example suite overview.
2. Create `design/examples/auth-web.md`.
3. Create `design/examples/web-spa.md`.
4. Create `design/examples/mobile.md`.
5. Create `design/examples/desktop.md`.
6. Create `design/examples/resource-api.md`.
7. Create `design/examples/client-profiles.md`.
8. Update `design/scope.md` to keep examples optional and non-core.
9. Update `design/migration.md` to point to the future examples architecture without committing to implementation.
10. Update client config docs with the future `client_type`, `allowed_origins`, and redirect-rule direction.
11. Update infra docs to describe example hosting as opt-in infrastructure only.
12. Add this slice to `design/implementation/ROADMAP.md`.
13. Run markdown/path sanity checks with `git diff --check`.

## Acceptance Criteria

- Example architecture docs exist under `design/examples`.
- Docs define `auth-web`, `web-spa`, `mobile`, `desktop`, and `resource-api`.
- Docs define `spa`, `native_mobile`, `native_desktop`, and `web_confidential` client profiles.
- Docs state that SPA and native apps are public clients and must use PKCE.
- Docs state that desktop loopback redirects require dynamic port support in a later code slice.
- Docs state that mobile/desktop examples use the external/system browser, not embedded WebViews.
- Docs state that example infra is opt-in and disabled by default.
- Docs preserve the API-only Irongate core boundary.
- No code or deployed infrastructure changes are made in this slice.

## Manual Validation

Manual validation is a design review:

- Read the new `design/examples` docs end to end.
- Confirm there is no language implying examples are required for Irongate core.
- Confirm there is no language implying browser/native clients can keep client secrets.
- Confirm every example has a clear security purpose.

## Next Slice

After this slice, define a code slice for the smallest auth-core change needed by the example architecture.

Likely next slice:

```text
28_client_profiles_and_redirect_rules
```

Expected scope:

- Add `client_type` to `auth.clients.toml`.
- Add `allowed_origins` for browser clients.
- Validate client profile config at startup.
- Add native desktop loopback redirect matching with dynamic port.
- Keep existing clients backward-compatible only if the design explicitly allows a default profile.
