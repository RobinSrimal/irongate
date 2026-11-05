# Apple Callback Rejection

Source: [packages/functions/auth/tests/oidc_apple_callback/rejection.rs](../../../../../packages/functions/auth/tests/oidc_apple_callback/rejection.rs).

## What

Form-post callback failure and bounded JWKS-refresh tests using the real router and validator.

## Why

Invalid Apple tokens must not resolve/update identities or issue codes, including after a key refresh.

## How

A controlled Apple client returns signed ID tokens and separate initial/refreshed key sets or a
refresh error. The existing state/session and local client-secret fixtures drive the actual callback.
Known-key signature, claim, algorithm and non-signing-use failures reject without forced refresh.
Missing keys, failed refresh, non-signing refreshed keys and bad nonce after refresh stop after one
attempt. Existing identities come from the parent fixture's real atomic provider issuance helper,
including its generation-bound code. Both new and existing identities retain identical
account/identity/index/code snapshots.

Responses remain safe invalid_grant errors without redirect/code/token; expected state/session
consumption is checked separately. A successful refresh uses valid signed verified email and produces a generation-bound code that
exchanges through /token. Counts bound refresh work but do not certify the reqwest cache, real Apple
service/device acceptance or DynamoDB semantics. No live credentials or network calls are used.
