# Google Callback Tests

Source: [packages/functions/auth/tests/oidc_google_callback.rs](../../../../packages/functions/auth/tests/oidc_google_callback.rs).

## What

Signed Google claims, identity resolution, callback state and first-party code exchange.

## Why

Google claims must not turn email equality into automatic account linking.

## How

Signed local RSA tokens exercise the real validator with shared
[validation cases](support/oidc_validation.md): accepted controls, independent claim errors, exact
key IDs, metadata/components and corrupted signatures. Tests retain valid Google claim parsing,
identity isolation, deleted-identity policy, state rejection and successful code exchange.

The [rejection child module](oidc_google_callback/rejection.md) checks one forced refresh, terminal
failure and unchanged account/identity/code state. These are not real Google acceptance tests,
HTTP cache tests or complete OIDC conformance certification.
