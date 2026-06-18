# 29_cors_for_browser_clients

## Goal

Apply the `allowed_origins` client configuration added in slice 28 to HTTP CORS responses.

At the end of this slice, browser-based public clients can call the auth API from explicitly configured origins, while unknown origins receive no CORS approval. This keeps browser integration possible without returning to permissive wildcard CORS.

## Design Docs Followed

This slice follows and updates:

- `design/auth/config/clients.md`
- `design/auth/config/client-file.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/api/providers/password.md`
- `design/examples/client-profiles.md`
- `design/implementation/ROADMAP.md`

## Scope Decision

In scope:

- Build CORS allowed origins from configured `allowed_origins`.
- Apply CORS to browser-relevant public auth routes.
- Handle preflight `OPTIONS` requests.
- Return exact allowed origins, never `*`.
- Do not enable credentialed browser CORS by default.
- Add focused route tests for allowed origin, unknown origin, and preflight.
- Update design docs to state that response CORS is now config-driven.

Out of scope:

- Building example apps.
- Deploying example infra.
- Cookie/session-based browser auth.
- BFF, token mediator, or DPoP.
- Per-client origin matching on individual token requests.
- Runtime client management.

## Acceptance Criteria

- A configured SPA origin receives `access-control-allow-origin` with the exact origin value.
- Unknown origins do not receive `access-control-allow-origin`.
- CORS responses do not use wildcard origins.
- CORS responses do not set `access-control-allow-credentials` by default.
- Preflight requests for configured origins succeed for supported browser methods.
- Preflight requests for unknown origins do not grant CORS access.
- Native/mobile/desktop clients do not need CORS to work.

## Tests

Focused tests:

```text
cors_allows_configured_browser_origin_without_wildcard
cors_rejects_unknown_origin
cors_preflight_for_token_uses_configured_origin
cors_preflight_rejects_unknown_origin
```

Full validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:infra
```

## Next Slice

After this slice, define the first optional example implementation only after choosing which example should come first.

Likely next slice:

```text
30_auth_web_example_foundation
```

That slice should remain optional and must not change the default auth-core deployment.
