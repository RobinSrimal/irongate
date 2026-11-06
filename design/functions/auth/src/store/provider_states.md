# Provider State

Source: [packages/functions/auth/src/store/provider_states.rs](../../../../../packages/functions/auth/src/store/provider_states.rs).

## What

One-use storage for outbound provider transactions.

## Why

Callbacks must recover the original session, nonce and upstream PKCE verifier only after satisfying
their provider-specific proof, without consuming a concurrent replacement.

## How

Creation validates lowercase HMAC digests, nonce/verifier presence, ordered timestamps and current
validity, then uses an absent-key Put with matching expiry/TTL. Google requires a browser digest;
Apple is an explicit binding variant. The raw state and Google browser proof are never persisted.

`take_provider_state` requires `GoogleBrowser(digest)` or `Apple`. A strong exact-key read captures
the raw value. Record validation, provider matching, constant-time browser-digest comparison and
runtime expiry checks precede a single conditional Delete of that exact value. Expiry is rechecked
immediately before dispatch using the injected provider clock. Wrong proof/provider and expired
records return no result without deleting them; TTL cleanup remains independent.

A confirmed conditional loss returns no result, without rereading or adopting a replacement.
Malformed stored records give sanitized storage errors; operational/conflict/uncertain failures
propagate. A lost response may follow committed deletion and is not retried or restored. Only
confirmed success returns callback metadata. Authorize-session consumption remains the caller's
next separate operation. This module does not accept a raw cookie or define Apple's browser policy.
