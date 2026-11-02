# Google OIDC Helpers

Source: [packages/functions/auth/src/providers/google.rs](../../../../../packages/functions/auth/src/providers/google.rs).

## What

Authorization URL construction, code exchange, identity digest and signed-ID-token validation.

## Why

Provider claims are accepted only after server-side verification.

## How

A reused reqwest client keeps JWKS for ten minutes behind an async lock; forced refresh is exposed to the callback. Validation requires RS256, a matching RSA key, issuer/audience, expiry, bounded future iat, nonce and nonempty subject. Current key selection does not enforce every JWK-use constraint or native presenter rules.
