# Apple Callback Tests

Source: [packages/functions/auth/tests/oidc_apple_callback.rs](../../../../packages/functions/auth/tests/oidc_apple_callback.rs).

## What

Signed provider-token validation, identity resolution, callback state and first-party code exchange.

## Why

Apple identity proof must remain bound to the original auth request.

## How

Local RSA fixtures exercise the real validator with shared
[validation cases](support/oidc_validation.md): accepted controls, independent claim errors, exact
key IDs, metadata/components and corrupted signatures. Tests retain valid Apple claim parsing,
provider errors, identity isolation, deleted-identity policy, state rejection and code exchange.

The [rejection child module](oidc_apple_callback/rejection.md) checks one forced refresh, terminal
failure and unchanged account/identity/code state. Real Apple service/device login and HTTP cache
freshness are not tested; these checks are not full OIDC conformance certification.
