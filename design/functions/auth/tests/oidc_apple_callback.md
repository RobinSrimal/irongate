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
provider errors, state rejection and code exchange. Existing identities are seeded through real
provider issuance preparation/finish with a bound session, the shared proof's Apple constructor and
fresh digest-keyed code. The fixture proof uses synthetic claims only at the store boundary.

The [issuance child](oidc_apple_callback/issuance.md) exercises signed email changes/omission and
ignored unsigned profile fields, verified-email ownership resolution, safe storage failures, replay and a real
commit-time account change. Duplicated resolve-only creation/reuse-policy checks are consolidated
into the shared [provider store contracts](provider_issuance.md), also run on disposable Local storage.

The [rejection child module](oidc_apple_callback/rejection.md) checks one forced refresh, terminal
failure and unchanged account/identity/code state. Real Apple service/device login and HTTP cache
freshness are not tested; these checks are not full OIDC conformance certification.

Application-state fixtures explicitly install the shared [captured audit emitter](support/audit.md),
so tests do not write production stdout/stderr. Dedicated audit route suites assert emission itself.
