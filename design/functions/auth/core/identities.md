# Identity Model

## What

Password, Google and Apple identities map to generated persisted subjects. The implementation is in
[AuthStore](../src/store/mod.md), [records](../src/store/records.md) and provider modules.

## Why

Provider issuer/subject is identity proof; an email claim is not a universal account key.

## How

Password identity creation follows verification. Google/Apple identities follow signed-token
validation and use issuer/subject HMAC lookup. Subsequent login resolves the stored subject rather
than deriving an ID from current claims. For a new Apple identity, signed verified email can resolve
an active email owner or create a fresh account/owner. Google remains separate. No existing accounts
are merged and there is no user-facing link/merge operation.

Password completion also commits an [email owner](../src/store/email_ownership.md) and subject
reverse index for the normalized-email HMAC digest. Pending requests do not claim ownership, and
active owners cannot be replaced or reused to enroll another password. Apple requires the active
owner's matching active account and reverse index, all guarded at commit. Missing or malformed state
fails closed. A verified relay address is its own address; unsigned profile data is never authority.
Password-first then Apple can share a subject; Apple-first password enrollment remains rejected.

Google/Apple preparation retains the exact identity and active-account generation without writes.
Finishing commits identity/account work and its first-party authorization code together, guarded
by those snapshots or absence for a new identity. Later changed/missing email claims cannot move
the identity, and issuance does not retry by selecting a different subject or newer generation.
Returning identities never reassign ownership from changed email. Pending password registrations
do not select an Apple account or install a password during provider sign-in.

Deleted-identity reuse never revives a deleted subject: Apple can attach to an active owner or
create a fresh account for an available email; Google/password create fresh subjects. See
[identity storage](../store/identities.md) for the transaction and tombstone details.
Password reuse requires eligible identity and email-owner tombstones. Ownership deletion clears
subject linkage and its reverse index without TTL. The template uses one current contract without
compatibility paths or ownership backfills.
