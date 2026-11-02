# Web OAuth Requests

Source: [packages/examples/web/src/oauth.ts](../../../../packages/examples/web/src/oauth.ts).

## What

PKCE generation, authorize URL construction and callback URL normalization.

## Why

All login methods use a common client transaction shape.

## How

Generates a verifier/challenge and includes state, nonce, scope, provider and S256 in authorize requests. Callback URL is /auth/callback under the configured base. These helpers construct requests, not token-validation decisions.
