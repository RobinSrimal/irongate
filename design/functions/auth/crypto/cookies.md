# Cookies

Implementation: [crypto/encrypt.rs](../src/crypto/encrypt.md).
Google binding: [API cookie helper](../src/api/providers/browser_binding.md).

## What And Why

The core cookie builder carries an opaque authorize-session identifier, not browser access or refresh
tokens. The web example owns its separate application-session cookie contract.

## How

`SecureCookie` defaults to HttpOnly, Secure, SameSite=Strict and Path=/.
The authorize handler sets a fixed 600-second max-age; this is not derived from the configured
authorize-session record TTL. Runtime record expiry remains the authorization check.

The same module includes RSA-OAEP/AES-GCM cookie-value encryption helpers, but the current authorize
cookie contains the opaque session key directly and does not call those helpers.

Google uses a separate per-transaction `__Host-ig-google-<state-HMAC>` cookie. Its independent
32-character random value is represented only by a purpose-separated browser HMAC in provider
state. Secure, HttpOnly, Path=/ and no Domain bind it to the issuer host; SameSite=Lax accommodates
Google's top-level GET callback. Lifetime is bounded by both provider-state TTL and the original
session deadline, not the fixed authorize-cookie lifetime above.

The callback requires exactly one matching cookie before consuming state/session or exchanging a
code. Duplicate, missing or invalid proofs reject without consumption. Confirmed state consumption
causes success/terminal-error responses to clear only that cookie with matching attributes and
Max-Age=0. Start/callback responses use no-store and no-referrer. Independent names support multiple
pending transactions; abandoned cookies expire. This is not application-session/SSO state and does
not replace client state, PKCE, nonce or provider validation. Apple uses its existing form-post flow,
not this Lax-cookie contract.
