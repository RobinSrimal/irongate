# Identity Storage

## What

Provider identity mappings and subject reverse indexes in [AuthStore](../src/store/mod.md) and
[records](../src/store/records.md). Google/Apple callback issuance uses the typed
[provider issuance module](../src/store/provider_issuance.md).

## Why

A verified provider identity needs a stable internal subject and indexed lifecycle cleanup.
Identity creation or metadata updates must commit with the callback's authorization code, not
survive a confirmed code-write rejection on their own.

## How

Password identities use a normalized-email HMAC; Google/Apple use purpose-separated issuer/subject
HMACs after signed-token validation. Equal email across providers or password login does not merge
accounts. Changed or missing email claims cannot move a returning provider identity to another subject.

`prepare_provider_issuance` accepts only Google/Apple, the validated identity digest/properties, a
consumed authorize-session snapshot, code TTL and existing reuse policy. It checks selected-provider
match and logical expiry, reads the identity consistently and validates its provider/digest binding.
An active identity must resolve to its existing matching active account; missing/inactive accounts
fail closed rather than creating another account. Preparation writes nothing and privately retains
the exact identity/account value and credential generation.

`finish_provider_issuance` constructs the code from those snapshots without rereading or adopting a
new generation. It updates last-seen/properties and performs one application-level transaction attempt:

| Identity State | Atomic Operations And Conditions |
| --- | --- |
| Absent or reusable deleted | Create fresh generation-zero account, identity, subject index and code. Account/index/code require absence; identity requires absence or the exact prepared tombstone. |
| Active | Check the complete prepared active account, replace the exact prepared identity, and create the code conditioned on absence. |

Persistent records have no TTL; the code has matching logical expiry and DynamoDB `expiry`.
The consumed session's expiry is checked again immediately before dispatch, using an injectable
issuance clock. Identity metadata keeps the callback's existing allowlist; code properties are
restricted to provider, email and email-verification flag.

Deletion strips subject/properties and records reuse timing. Reuse honors the existing policy and
required tombstone metadata, always generating a new subject. A changed tombstone, competing claim,
stale account/identity or code collision rejects the transaction without partial issuance writes.
There is no automatic re-resolution, retry planner, email ownership index, linking or account merge.

Earlier provider-state/session consumption and external exchange are not part of this transaction.
Failed callbacks require fresh sign-in. Confirmed conditional rejection is distinct from an
operational error, transaction conflict or uncertain response; an error is not proof of rollback,
and no compensating account deletion is performed. Exact-value guards detect changed state at commit,
not disable/enable history that restores the same value. See [lifecycle](../core/account-lifecycle.md).
