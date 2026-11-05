# Identity Model

## What

Password, Google and Apple identities map to generated persisted subjects. The implementation is in
[AuthStore](../src/store/mod.md), [records](../src/store/records.md) and provider modules.

## Why

Provider issuer/subject is identity proof; an email claim is not a universal account key.

## How

Password identity creation follows verification. Google/Apple identities follow signed-token
validation and use issuer/subject HMAC lookup. Subsequent login resolves the stored subject rather
than deriving an ID from current claims. Password and provider logins sharing an email currently
create separate accounts; there is no user-facing link/merge operation.

Password completion also commits an [email owner](../src/store/email_ownership.md) and subject
reverse index for the normalized-email HMAC digest. Pending requests do not claim ownership, and
active owners cannot be replaced or reused to enroll another password. Provider callbacks neither
read nor write this index. It is not a shared-account resolver or a cross-provider uniqueness rule.

Google/Apple preparation retains the exact identity and active-account generation without writes.
Finishing commits identity/account work and its first-party authorization code together, guarded
by those snapshots or absence for a new identity. Later changed/missing email claims cannot move
the identity, and issuance does not retry by selecting a different subject or newer generation.
This is a transaction boundary, not shared-account resolution or email-ownership verification.

Deleted-identity reuse creates a fresh subject under the configured policy. See
[identity storage](../store/identities.md) for the transaction and tombstone details.
Password reuse requires eligible identity and email-owner tombstones. Ownership deletion clears
subject linkage and its reverse index without TTL; there is no historical ownership backfill.
