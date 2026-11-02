# Google Callback Tests

Source: [packages/functions/auth/tests/oidc_google_callback.rs](../../../../packages/functions/auth/tests/oidc_google_callback.rs).

## What

Signed Google claims, identity resolution, callback state and first-party code exchange.

## Why

Google claims must not turn email equality into automatic account linking.

## How

Signed local RSA tokens and a fake OIDC client exercise callback success/failure, missing keys/state, a JWKS refetch and deleted-identity policy. These are not real Google acceptance tests or complete independent claim-rejection coverage.
