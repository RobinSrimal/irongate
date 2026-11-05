# Shared Provider Issuance

Source: [packages/functions/auth/tests/support/provider_issuance.rs](../../../../../packages/functions/auth/tests/support/provider_issuance.rs).

## What

Backend-neutral fixture and case macro for atomic provider identity/code and Apple ownership tests.

## Why

Memory and DynamoDB Local must enforce the same account/identity snapshots, code bindings and
all-or-nothing writes without depending on transaction ordering or mock success semantics.

## How

Fixtures use real HMAC identity/code key families, a consumed-session snapshot with explicit
client/redirect/scope/PKCE/nonce bindings, and a controllable issuance-only clock. Active and deleted
states are seeded through actual issuance, code consumption and account-deletion APIs. Conditional
replacements use the supplied adapter's compare-and-set rather than fabricated conflict responses.

The existing transaction gate pauses actual adapter transactions. Helpers discover the candidate
subject from its code operation, compare key sets rather than operation order, and read exact
account/identity/index/code rows. Successful codes must be usable with the expected generation,
client bindings, logical TTL and narrow claims, separately from current identity metadata.
Apple fixtures derive an opaque email proof from synthetic post-validation claims bound to the
actual Apple issuer/subject digest. Signed callback tests, not these store fixtures, verify JWTs.
Seed emails are identity-specific so unrelated cases do not accidentally share ownership. Key
snapshots include owner/email indexes for new Apple identities. The outcome, race, Apple ownership
and Apple race children own assertions; both runners instantiate the same macro.
