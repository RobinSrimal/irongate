# Provider Ownership Contracts

Source: [packages/functions/auth/tests/support/provider_issuance/ownership.rs](../../../../../../packages/functions/auth/tests/support/provider_issuance/ownership.rs).

## What

Shared memory/DynamoDB Local contracts for Google/Apple email authority and account resolution.

## Why

Only a proven email can select an account for a new provider identity. Existing identities must not
move, pending credentials must not install themselves and deletion must preserve ownership safety.

## How

Fixtures complete actual password registration with the same HMAC secret as provider issuance.
Both providers join that owner with normalized authoritative email, preserving the password and ownership
records. Returning changed/missing/unverified email stays on the original subject, even when another
active owner holds the later address. A literal relay address creates separate ownership, and a new
identity from the other provider can join the original owner through its own proof.

Rejections isolate missing proof, malformed/unverified email, wrong identity and wrong provider
bindings, including on returning identities. Active owners with missing/inactive/mismatched accounts or malformed/missing indexes
cannot trigger alternate account creation or repair. A gated owner-read outage remains operational.
Controlled clocks cover owner reuse policy and exact retention boundaries. Linked deletion removes
both identities' subject indexes and password material; a later owner and reused provider identity
survive an old deletion retry. New rows use existing HMAC families; no compatibility fixtures or
backfills are added. These store cases assume upstream validation; callback tests verify real JWTs.
