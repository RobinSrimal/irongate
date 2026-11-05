# Web BFF Routes

Source: [packages/examples/web/src/routes.ts](../../../../packages/examples/web/src/routes.ts).

## What

Email-first registration, deliberate password setup after email proof, password/provider login,
callback, signed-in view and logout.

## Why

The browser receives opaque session cookies while Worker storage holds tokens. Registration does
not choose a password before email ownership proof, and opening a verification link cannot activate
an account or create a session.

## How

- `GET /auth/register` renders an email-only form; its POST forwards only `{email}` to
  `/password/register`.
- `GET /auth/verify-email?token=...` renders a password/confirmation form without upstream or session
  access. Missing, empty or longer-than-1,024-character proofs fail with a fixed error before
  rendering. HEAD follows the same checks but returns no body. Refresh and prefetch cannot consume
  the proof.
- `POST /auth/verify-email` requires a nonempty string proof of at most 1,024 characters, a nonempty
  password and matching confirmation. It forwards only `{token, new_password}` to `/password/verify`;
  the query string is not a proof fallback. Passwords are not trimmed, and core password policy stays
  authoritative. Completion shows a sign-in link without logging in or accessing session storage.
- Both paths require the request origin to match the effective web base URL, which must use HTTP(S)
  without URL credentials. With `WEB_BASE_URL` unset, existing request-origin inference remains.
  POST additionally requires a single serialized, matching `Origin`; missing, `null`, foreign or
  malformed values fail before parsing or forwarding the form. Other methods return 405 with Allow.
- These paths retain no-store, CSP and escaping, and use `Referrer-Policy: strict-origin` so browser
  same-origin POSTs remain usable without sending the proof-bearing path/query or a downgrade
  referrer. Errors never echo submitted fields, upstream bodies or exception text.

Callbacks compare cookie-bound state, exchange with PKCE and replace the login record with an app
session. Current routes do not enforce stored expiresAt, validate ID-token nonce or rotate refresh
tokens. Origin enforcement above is limited to registration/completion, not login/logout; no general
CSRF-token mechanism is implemented. Logout attempts upstream revoke before deleting the session;
no reset-password page is mounted.
