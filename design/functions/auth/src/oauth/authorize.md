# Authorize Endpoint

Source: [packages/functions/auth/src/oauth/authorize.rs](../../../../../packages/functions/auth/src/oauth/authorize.rs).

## What

Validation and storage of a client authorization request.

## Why

Login starts only for registered clients, allowed redirects/scopes and configured providers.

## How

Requires response_type=code and provider selection, checks PKCE policy and S256, then creates a short-lived HMAC-keyed session. OIDC nonce is retained for openid. Returns an API continuation redirect and an opaque session cookie, not a hosted login page or tokens.
