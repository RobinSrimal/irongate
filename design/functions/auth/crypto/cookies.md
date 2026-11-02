# Cookies

Implementation: [crypto/encrypt.rs](../src/crypto/encrypt.md).

## What And Why

The core cookie builder carries an opaque authorize-session identifier, not browser access or refresh
tokens. The web example owns its separate application-session cookie contract.

## How

`SecureCookie` defaults to HttpOnly, Secure, SameSite=Strict and Path=/.
The authorize handler sets a fixed 600-second max-age; this is not derived from the configured
authorize-session record TTL. Runtime record expiry remains the authorization check.

The same module includes RSA-OAEP/AES-GCM cookie-value encryption helpers, but the current authorize
cookie contains the opaque session key directly and does not call those helpers.
