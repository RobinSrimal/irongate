# Google Callback Rejection

Source: [packages/functions/auth/tests/oidc_google_callback/rejection.rs](../../../../../packages/functions/auth/tests/oidc_google_callback/rejection.rs).

## What

Callback failure and bounded JWKS-refresh tests using the real router and provider validator.

## Why

Rejected tokens must not mutate identity/account state or issue codes, and key refresh must not
turn an ordinary signature/claim failure into a retry loop.

## How

A controlled Google client returns signed tokens and separate initial/refreshed key sets or a
refresh error. Valid provider state/session ensures validation is reached. Known-key signature,
claim, algorithm and non-signing-use failures reject without forced refresh. Missing keys, failed
refresh, non-signing refreshed keys and bad nonce after refresh reject after one attempt.

Each rejection runs with a new and an existing identity, comparing account/identity/index/code
snapshots without dumping their values. Safe invalid_grant responses carry no redirect/code/token;
provider state/session are consumed as expected. A successful refresh issues a generation-bound code
that exchanges through /token. Fetch counts prove the retry bound alongside actual outcomes, not
network cache freshness, real Google availability or DynamoDB semantics.
