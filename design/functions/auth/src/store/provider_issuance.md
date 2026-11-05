# Atomic Provider Issuance

Source: [packages/functions/auth/src/store/provider_issuance.rs](../../../../../packages/functions/auth/src/store/provider_issuance.rs).

## What

Typed preparation and finishing operations for Google/Apple identity resolution and first-party
authorization-code issuance. Prepared state has private fields and is consumed by finishing.

## Why

The provider identity, active account generation and issued code must share a transaction boundary.
A canceled code write must not leave new accounts/indexes or partially updated identity metadata.

## How

The callback supplies the provider, HMAC issuer/subject digest and allowlisted identity properties
only after full provider-token validation. Preparation rejects password, empty digests,
provider/session mismatch and expired sessions. It validates persisted identity provider/digest
bindings and captures the active account and exact identity value for a returning subject. Missing
or inactive accounts fail closed rather than creating replacements.

Absent or eligible deleted identities get fresh generation-zero subjects. Reuse follows the
existing tombstone policy and retention clock; there is no email index or cross-provider linking.
Preparation writes nothing and cannot accept caller-selected subjects or generations.

Finishing checks session expiry, sets last-seen, and derives the code from that private snapshot:

- New/reused identity: one transaction creates account, subject index, identity and code. Each
  creation requires absence; tombstone replacement requires the complete prepared value.
- Returning identity: one transaction checks the complete active-account value, replaces the
  exact identity and creates the absent code. It does not reread/adopt another generation.

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
transaction. Failed callbacks restart sign-in. The module does not change JWT verification, code
consumption, account deletion, email ownership or already-issued access-token semantics.
