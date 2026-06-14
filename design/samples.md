# Sample Applications

This document describes how example apps should relate to the auth foundation.

## Decision

The core repository should stay frontend-agnostic.

The auth foundation provides Rust + AWS auth/OIDC backend primitives. It should not force a frontend framework, hosted UI design system, or application hosting model.

Sample apps can demonstrate complete login flows, but they should live outside the auth core. A separate repository is the cleanest default.

## Why Separate

Keeping samples separate preserves the template's main promise:

```text
Use this auth backend with your own app, frontend, and deployment choices.
```

A bundled hosted UI would imply product and framework choices that many template users do not want:

- React, Vue, Svelte, native mobile, or another UI stack.
- A styling system and component model.
- Hosted login page routing.
- Consent and account-selection UX.
- Frontend deployment and hosting assumptions.

Those decisions are valuable for an example, but they should not become requirements of the auth foundation.

## Suggested Sample Repo

A separate sample app can show:

- Register with email and password.
- Verify email through a Resend-delivered link.
- Login with password.
- Login with Google.
- Login with Apple.
- Perform authorization-code + PKCE flow.
- Exchange code for access, ID, and refresh tokens.
- Refresh tokens.
- Call `/userinfo`.
- Call a protected API route that uses access-token `sub` for row-level access control.
- Demonstrate logout by clearing local app state and calling `/oauth/revoke` for the refresh token.

The sample should use the same public auth APIs that any application would use. It must not rely on private store access, raw DynamoDB reads, or internal Rust modules.

## Relationship To Hosted UI

Hosted UI is out of the initial core, but a sample app can act as a reference hosted-login experience.

If hosted UI becomes a product goal later, it should be designed as an optional layer on top of the auth foundation, with its own security design for:

- Browser sessions.
- CSRF protection.
- Login and registration forms.
- Password reset forms.
- Provider selection.
- Consent.
- Account selection.
- Error pages.

## Security Invariants

- Samples must not weaken the auth core boundary.
- Samples must not require direct access to `AuthTable`.
- Samples must not require runtime admin/client management APIs.
- Sample frontend choices must not become required dependencies of the auth foundation.
