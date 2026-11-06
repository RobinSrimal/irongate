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
HMACs after signed-token validation. Existing accounts are never merged. New identities can
resolve an email owner through [provider email authority](../src/providers/verified_email.md). Changed or missing
email claims cannot move a returning provider identity to another subject.

Password registration additionally writes an [email owner](../src/store/email_ownership.md) and
its subject reverse index only after email proof completion. These use the normalized-email HMAC
family independently of the password-identity digest. Active ownership denies completion rather
than enrolling a password onto an existing subject. Pending users do not claim an address.

`prepare_provider_issuance` accepts only Google/Apple, provider-bound authority, the validated identity digest/properties, a
consumed authorize-session snapshot, code TTL and existing reuse policy. It checks selected-provider
match and logical expiry, reads the identity consistently and validates its provider/digest binding.
An active identity must resolve to its existing matching active account; missing/inactive accounts
fail closed rather than creating another account. Preparation writes nothing and privately retains
the exact identity/account value and credential generation.

`finish_provider_issuance` constructs the code from those snapshots without rereading or adopting a
new generation. It updates last-seen/properties and performs one application-level transaction attempt:

| Identity State | Atomic Operations And Conditions |
| --- | --- |
| Absent or reusable deleted Google/Apple | Require typed email authority bound to this provider/identity. Guard/create account, email owner and email index, create identity index, conditionally write identity and create code in six operations. Existing owners require exact active account and matching index; available email gets a fresh subject. |
| Active | Check the complete prepared active account, replace the exact prepared identity, and create the code conditioned on absence. |

Persistent records have no TTL; the code has matching logical expiry and DynamoDB `expiry`.
The consumed session's expiry is checked again immediately before dispatch, using an injectable
issuance clock. Identity metadata keeps the callback's existing allowlist; code properties are
restricted to provider, email and email-verification flag.

Deletion strips subject/properties and records reuse timing. Reuse honors the existing policy and
required tombstone metadata, never reviving a deleted subject. A changed tombstone, competing claim,
stale account/identity or code collision rejects the transaction without partial issuance writes.
New Google/Apple identities consult and conditionally claim email ownership; active identities do not.
There is no automatic re-resolution, retry planner or account merge. Password reuse checks both identity and
email-owner tombstones; deletion clears owner subject linkage and its exact reverse index together.
Ownership has no TTL or backfill and does not certify general cross-provider uniqueness. Password
completion still rejects active owners; it cannot install a pending password on a provider account.
Public Google callbacks without authoritative email reject without a separate-account fallback.
The [confirmation store](../src/store/sign_in_confirmation.md) captures the same private new-identity
plan and consumes browser/email proof atomically with six issuance operations and exact pending
deletion. It reuses [provider planning](../src/store/provider_plan.md), not a second resolver. There
is no public delivery or confirmation endpoint.

Earlier provider-state/session consumption and external exchange are not part of this transaction.
Failed callbacks require fresh sign-in. Confirmed conditional rejection is distinct from an
operational error, transaction conflict or uncertain response; an error is not proof of rollback,
and no compensating account deletion is performed. Exact-value guards detect changed state at commit,
not disable/enable history that restores the same value. See [lifecycle](../core/account-lifecycle.md).
