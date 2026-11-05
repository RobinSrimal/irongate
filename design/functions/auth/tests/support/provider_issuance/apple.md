# Apple Ownership Contracts

Source: [packages/functions/auth/tests/support/provider_issuance/apple.rs](../../../../../../packages/functions/auth/tests/support/provider_issuance/apple.rs).

## What

Shared memory/DynamoDB Local contracts for Apple email authority and account resolution.

## Why

Only a proven email can select an account for a new Apple identity. Existing identities must not
move, pending credentials must not install themselves and deletion must preserve ownership safety.

## How

Fixtures complete actual password registration with the same HMAC secret as provider issuance.
Apple joins that owner with a normalized verified email, preserving the password and ownership
records. Returning changed/missing/unverified email stays on the original subject, even when another
active owner holds the later address. A relay address creates separate ownership; Google stays separate.

Rejections isolate missing proof, malformed/unverified email, wrong identity and wrong provider
bindings. Active owners with missing/inactive/mismatched accounts or malformed/missing indexes
cannot trigger alternate account creation or repair. A gated owner-read outage remains operational.
Controlled clocks cover owner reuse policy and exact retention boundaries. Linked deletion removes
both identities' subject indexes and password material; a later owner and reused Apple identity
survive an old deletion retry. New rows use existing HMAC families; no compatibility fixtures or
backfills are added. These store cases assume upstream validation; callback tests verify real JWTs.
