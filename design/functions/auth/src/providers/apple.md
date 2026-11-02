# Apple OIDC Helpers

Source: [packages/functions/auth/src/providers/apple.rs](../../../../../packages/functions/auth/src/providers/apple.rs).

## What

Apple URL/code exchange, client-secret JWT generation, JWKS caching and ID-token validation.

## Why

Apple credentials and provider claims have different signing keys and audiences from Irongate tokens.

## How

Client-secret JWTs use the configured ES256 private key/team/key ID. Apple ID tokens use RS256 JWKS validation with issuer/audience/expiry/iat/nonce/subject checks. The shared client caches JWKS for ten minutes and exposes forced refresh; string and boolean email flags are accepted.
