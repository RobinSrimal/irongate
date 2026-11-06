# Google Browser Binding Tests

Source: [packages/functions/auth/tests/oidc_google_callback/browser_binding.rs](../../../../../packages/functions/auth/tests/oidc_google_callback/browser_binding.rs).

## What

HTTP contracts for Google transaction-cookie admission, cleanup and concurrent callbacks.

## Why

Valid provider state alone must not authorize a callback, and proof rejection must not consume
another browser's session or contact Google.

## How

The real router/store receives missing, wrong, empty, short, malformed and duplicate cookies,
other-transaction/old cookies and query substitutes. Unbound Google and Apple records also reject.
State/session snapshots remain unchanged, no codes are created and upstream exchange/JWKS are
not reached. Restoring a valid binding and cookie completes the same transaction with a signed token.

Two actual starts generate independent cookies and state, cap expiry by session/provider TTL,
store only browser HMACs and keep the raw proof out of Google's URL. Both completion orders work
with a multi-cookie jar; only the consumed cookie is cleared. Successful and rejected responses
check no-store/no-referrer, host-only scope, Secure, HttpOnly and Lax. Bound declines and invalid
provider tokens clear the cookie after consumption. A gate orders competing valid callbacks through
the real state transaction: one wins and exchanges, while the other fails without issuing a code.
These are HTTP fixtures, not browser cookie enforcement or live Google/device acceptance.
