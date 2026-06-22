# Auth Function Design

This folder describes the public auth function and shared auth library structure.

Target code:

```text
packages/functions/auth/src/
  api/
  core/
  store/
  crypto/
  providers/
  email/
  config/
  observability/
```

## Target Core

The first production-ready auth function should be narrow:

- OAuth authorize, token, discovery, JWKS, and userinfo.
- OpenID Connect-compatible discovery and ID-token issuance.
- Password identity with email verification and reset.
- Google OIDC identity.
- Apple OIDC identity.
- Refresh token rotation.
- DynamoDB-only auth store.
- Rate limiting.
- Email delivery for verification and reset flows.
- Configurable email templates for verification and reset messages.

The auth function is API-only. It should not render login, registration, reset, consent,
account-selection, or provider-selection pages. Application developers own their product UI and call
the auth endpoints from their app.

Optional examples live outside the auth function under `packages/examples` and `infra/examples`.
They must not make frontend framework or hosting choices part of the core auth runtime.

## Runtime Boundary

The auth function exposes OAuth/OIDC, password, Google, Apple, refresh, revoke, and userinfo routes.
Runtime control-plane operations live in the separate IAM-protected admin function.

## Admin Boundary

IAM-protected account lifecycle routes are served by the separate admin function documented in
`../admin`. The auth crate may contain shared domain/store modules used by that entrypoint, but the
public auth Lambda must not mount admin routes behind `$default`.

## Security Rule

The auth code should make security boundaries explicit in names and types. A caller should not need to remember that a generic `get` plus `remove` means "consume once"; the API should expose a `take_*` operation.
