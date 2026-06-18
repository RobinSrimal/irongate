# 28_client_profiles_and_redirect_rules

## Goal

Implement the smallest auth-core change needed by the optional web, mobile, and desktop example architecture.

At the end of this slice, Irongate client configuration should understand browser, native mobile, native desktop, and confidential web client profiles. Redirect validation should remain exact for ordinary clients and support dynamic loopback ports only for native desktop clients.

## Design Docs Followed

This slice follows and updates:

- `design/examples/client-profiles.md`
- `design/examples/web-spa.md`
- `design/examples/mobile.md`
- `design/examples/desktop.md`
- `design/auth/config/clients.md`
- `design/auth/config/client-file.md`
- `design/auth/core/clients.md`
- `design/auth/api/oauth/authorize.md`
- `design/auth/api/oauth/token.md`
- `design/implementation/ROADMAP.md`

## Scope Decision

In scope:

- Add explicit client profiles:
  - `spa`
  - `native_mobile`
  - `native_desktop`
  - `web_confidential`
- Keep `public` and `confidential` as legacy config aliases.
- Add `allowed_origins` parsing and startup validation for browser clients.
- Require PKCE for public browser/native profiles.
- Reject client secrets for public browser/native profiles.
- Support native desktop loopback redirects with dynamic runtime ports.
- Keep exact redirect matching for non-desktop clients.
- Update `auth.clients.toml`.
- Add focused Rust tests.
- Update design docs.

Out of scope:

- Building example apps.
- Deploying example infra.
- Implementing CORS response headers from `allowed_origins`.
- Adding BFF, token mediator, or DPoP.
- Changing token issuance semantics beyond profile-aware client validation.

## Acceptance Criteria

- `spa`, `native_mobile`, and `native_desktop` are public clients and require PKCE.
- `web_confidential` requires confidential client auth.
- `spa` clients require at least one `allowed_origins` entry.
- `allowed_origins` are validated as origins, not redirect URIs.
- Native mobile clients can register claimed HTTPS or reverse-domain private-use custom-scheme redirects.
- Native desktop clients can register loopback redirects without fixed ports.
- Native desktop authorize requests can use the registered loopback host/path with a dynamic port.
- Non-desktop clients still require exact redirect URI matching.
- Wildcard redirect URIs and wildcard origins are rejected.
- `auth.clients.toml` uses the new `spa` profile.

## Tests

Focused tests:

```text
client_config_accepts_profile_clients_and_browser_origins
client_config_rejects_invalid_profile_shapes
native_desktop_allows_dynamic_loopback_port
non_desktop_clients_keep_exact_redirect_matching
```

Full validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
```

## Next Slice

After this slice, define an example implementation slice only after deciding which example should be built first.

Likely first example implementation:

```text
29_auth_web_example_foundation
```

That slice should remain optional and must not change the default core deploy.
