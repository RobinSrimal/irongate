# Shared Registration Contracts

Source: [packages/functions/auth/tests/support/registration_contract.rs](../../../../../packages/functions/auth/tests/support/registration_contract.rs).

## What

Eight grouped registration contracts instantiated against memory and disposable DynamoDB Local.

## Why

Proof consumption, initial credential creation and both subject indexes must commit atomically.
Returned errors alone cannot establish rollback, and a stale proof must never replace a winner's
password or activate a later registration instance.

## How

Fixtures use typed pending/proof APIs, the actual email/identity/verification HMAC families, and
real Argon2id hashes. Six independent condition conflicts preserve every candidate row. Bounded
transaction gates order same-proof and sibling-proof completions with different passwords; exact
candidate account/index reads exclude loser orphans, and the recovery fixture's domain login
helper verifies that only the winning password authenticates. The shared helper is imported as
the crate-root `recovery_fixture` module in both runners. Recovery reuses
`crate::support::registration`, so each runner includes one shared registration fixture through
`mod support`. The supplied storage adapter remains memory or DynamoDB Local respectively.

Other cases order proof issuance against completion in both directions, replace an inspected
identity tombstone, and complete/delete/recreate a user before rejecting the old prepared binding.
Only the registration clock advances for expiry at preparation, finish and immediately before
dispatch; backing rows remain present. Pre-dispatch failure preserves proof/state and permits a
fresh completion, while a deliberately lost response after real commit leaves the account active
and the consumed proof unusable. Tests inspect observable stored results, not fabricated backend
success. The suite does not claim exhaustive concurrency or production network certification.
