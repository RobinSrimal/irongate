# Google Callback Issuance

Source: [packages/functions/auth/tests/oidc_google_callback/issuance.rs](../../../../../packages/functions/auth/tests/oidc_google_callback/issuance.rs).

## What

HTTP-level identity continuity, isolated accounts, issuance failures and callback replay regressions.

## Why

Correct store transactions alone do not prove that signed claims, redirects and error classification
are wired correctly through the provider callback.

## How

Requests use signed RSA ID tokens, the real validator/router and persisted state/session fixtures.
Returning signed email changes and omission retain the original subject; code claims stay limited
to provider/email/email_verified. Matching emails on another signed subject or a password account
do not link subjects. Signed name/picture metadata stays on the identity and does not enter code claims.

The shared [fault adapter](../support/provider_callback.md) injects conditional/existence failures
and operational/conflict failures at both preparation and finish. Responses must be exact sanitized
400 invalid_grant or 500 server_error bodies, with no redirect or cookie and no issuance retry.

The unchanged transaction gate pauses real issuance. A pre-callback snapshot must still match at
the gate, rejecting premature identity/account/code writes while ignoring consumed state/session.
The account is then disabled; the stale transaction must preserve that post-disable snapshot,
including identity metadata and codes. Pre-dispatch failure leaves all
candidate keys absent, whereas a deliberately lost response after a real commit leaves each planned
account/identity/index/code value intact. Both return server_error without a code redirect.

Success and all failure cases assert consumed state/session and reject replay without further
credential writes. Earlier consumption is not rolled back. Tests use memory storage, not live
provider services; the shared store/Local contracts establish adapter-level atomicity.
