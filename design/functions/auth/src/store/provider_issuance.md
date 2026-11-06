# Atomic Provider Issuance

Source: [packages/functions/auth/src/store/provider_issuance.rs](../../../../../packages/functions/auth/src/store/provider_issuance.rs).

## What

Typed preparation and finishing operations for Google/Apple identity resolution and first-party
authorization-code issuance. Prepared state has private fields and is consumed by finishing.

## Why

The provider identity, active account generation and issued code must share a transaction boundary.
A canceled code write must not leave new accounts/indexes or partially updated identity metadata.

## How

The callback supplies the provider, HMAC issuer/subject digest, allowlisted identity properties and
optional opaque `ProviderVerifiedEmail` only after full provider-token validation. Preparation rejects
a proof supplied for another identity or provider, password, empty digests,
provider/session mismatch and expired sessions. It validates persisted identity provider/digest
bindings and captures the active account and exact identity value for a returning subject. Missing
or inactive accounts fail closed rather than creating replacements.

An active identity is resolved before email, retaining its subject even when later email is missing,
unverified or belongs to another account. A new/reusable Google or Apple identity requires
[provider email authority](../providers/verified_email.md) and resolves through
the [email owner](email_ownership.md): either a guarded active owner/account/index or a fresh account
and owner. New identity metadata uses the proof's normalized email and verified flag, not arbitrary
properties as authority. Identity and owner tombstones independently enforce reuse policy.
Preparation writes nothing and cannot accept caller-selected subjects or generations. Its private
`AccountPlan` distinguishes returning identities from new identities with an ownership plan.
The new branch uses the private [provider plan](provider_plan.md), also used by durable confirmation.
The module's internal code-operation builder and checked lifetime helper preserve the same narrow
code claims for both paths. Prepared direct issuance remains non-deserializable and opaque.

Finishing checks session expiry, sets last-seen, and derives the code from that private snapshot:

- Returning identity: one transaction checks the complete active-account value, replaces the
  exact identity and creates the absent code. It does not reread/adopt another generation.
- New/reused Google or Apple identity: six operations guard/create account, owner and email index, create the
  identity subject index, conditionally write the identity and create the code. Existing ownership
  uses exact owner/index/account checks; available ownership uses absence/exact eligible tombstone
  conditions. A new identity gets its index even when the account already exists.

Persistent records have no TTL. The code has matching logical expiry and DynamoDB TTL using a
positive, checked configured duration. Its properties contain only provider, signed email and
email_verified; profile metadata stays in the identity. Client/redirect/scope/PKCE/OIDC nonce are
copied from the consumed session, not independently supplied values. Identity email changes never
change the subject. Raw codes never enter this module, only their lookup digest.

Expiry is checked again immediately before dispatch using the provider-issuance clock. This is not
a server-side deadline or a history marker for disable/enable changes that restore the same value.
There is one application transaction attempt, no compensation and no retry against newer snapshots.
Conditional failures and operational/conflict/uncertain errors retain their storage classification.
A lost response can follow a complete commit; the caller must not infer rollback from an error.

Provider state/session consumption and external exchange happen earlier and are not part of this
transaction. Failed callbacks restart sign-in. Pending password requests cannot supply authority or
install credentials during provider sign-in. Password enrollment into an existing account is rejected,
and existing accounts are never merged. Direct Google issuance without authoritative email rejects;
there is no separate-account fallback. [Confirmation](sign_in_confirmation.md) provides a separate
store-only proof boundary, not a public callback flow. There are no compatibility paths
or backfills. JWT verification, code consumption and access-token expiry semantics are unchanged.
