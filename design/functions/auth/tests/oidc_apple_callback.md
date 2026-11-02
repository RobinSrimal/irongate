# Apple Callback Tests

Source: [packages/functions/auth/tests/oidc_apple_callback.rs](../../../../packages/functions/auth/tests/oidc_apple_callback.rs).

## What

Signed provider-token validation, identity resolution, callback state and first-party code exchange.

## Why

Apple identity proof must remain bound to the original auth request.

## How

Local RSA fixtures and a fake OIDC client exercise valid/rejected claims, missing keys/state, one JWKS refetch, provider errors and no email auto-linking. Real Apple service/device login is not performed; not every claim has an independent rejection case.
