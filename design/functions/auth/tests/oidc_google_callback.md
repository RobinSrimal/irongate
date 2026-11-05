# Google Callback Tests

Source: [packages/functions/auth/tests/oidc_google_callback.rs](../../../../packages/functions/auth/tests/oidc_google_callback.rs).

## What

Signed Google claims, identity resolution, callback state and first-party code exchange.

## Why

Google identity and current email authority must be validated separately before account selection.

## How

Signed local RSA tokens exercise the real validator with shared
[validation cases](support/oidc_validation.md): accepted controls, independent claim errors, exact
key IDs, metadata/components and corrupted signatures. Tests retain valid Google claim parsing,
state rejection and successful code exchange. Existing-identity fixtures use real provider issuance
preparation/finish with a bound session, provider/identity-bound Workspace authority and a fresh
digest-keyed code. Authority is constructed from synthetic claims only in store fixture setup.

The [issuance child](oidc_google_callback/issuance.md) tests stable subjects for changed/missing signed
email, safe storage failure responses, replay and a real commit-time account
change. Duplicated resolve-only creation/reuse-policy checks are consolidated into the shared
[provider store contracts](provider_issuance.md), which also run against disposable Local storage.

The [rejection child module](oidc_google_callback/rejection.md) checks one forced refresh, terminal
failure and unchanged account/identity/code state. These are not real Google acceptance tests,
HTTP cache tests or complete OIDC conformance certification.

The [ownership child](oidc_google_callback/ownership.md) proves signed authority, owner sharing,
third-party-email rejection, unsigned-parameter rejection and returning identity precedence.

Application-state fixtures explicitly install the shared [captured audit emitter](support/audit.md),
so tests do not write production stdout/stderr. Dedicated audit route suites assert emission itself.
