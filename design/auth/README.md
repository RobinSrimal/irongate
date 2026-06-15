# Auth Design

This folder describes the target auth code structure.

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

The first production-ready core should be narrow:

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
- IAM-protected account lifecycle operations.
- Configurable deleted identity reuse policy.

The auth core is API-only. It should not render login, registration, reset, consent, account-selection, or provider-selection pages. Application developers own their product UI and call the auth endpoints from their app.

App and UI decisions are intentionally deferred. The repo should not assume an app framework or app hosting model until that is explicitly designed later.

## Out Of Initial Core

- Public/custom-key runtime admin API.
- Public bootstrap.
- Payments.
- Email OTP or magic-link login.
- Generic arbitrary OAuth2 identity.
- Built-in HTML auth UI.

Those areas are documented in `../scope.md` so their exclusion is intentional without creating non-code-shaped auth folders.

## Security Rule

The auth code should make security boundaries explicit in names and types. A caller should not need to remember that a generic `get` plus `remove` means "consume once"; the API should expose a `take_*` operation.
