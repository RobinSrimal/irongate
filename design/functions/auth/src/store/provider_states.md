# Provider State

Source: [packages/functions/auth/src/store/provider_states.rs](../../../../../packages/functions/auth/src/store/provider_states.rs).

## What

One-use storage for outbound provider transactions.

## Why

Callbacks must recover the original session, nonce and upstream PKCE verifier.

## How

Conditional creation stores expires_at and TTL. Take rejects expired state and uses a conditional Delete; the caller checks provider/session binding. The state key is a digest, while the server-side record contains the verifier needed for code exchange.
