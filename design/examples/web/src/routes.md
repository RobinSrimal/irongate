# Web BFF Routes

Source: [packages/examples/web/src/routes.ts](../../../../packages/examples/web/src/routes.ts).

## What

Registration, email verification, password/provider login, callback, signed-in view and logout.

## Why

The browser receives opaque session cookies while Worker storage holds tokens.

## How

Callbacks compare cookie-bound state, exchange with PKCE and replace the login record with an app session. GET verification consumes a link immediately. Current routes do not enforce stored expiresAt, validate ID-token nonce, rotate refresh tokens or check an Origin allowlist/CSRF token. Logout attempts upstream revoke before deleting the session; no reset-password page is mounted.
