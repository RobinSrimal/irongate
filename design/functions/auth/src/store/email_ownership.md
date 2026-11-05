# Verified Email Ownership

Source: [packages/functions/auth/src/store/email_ownership.rs](../../../../../packages/functions/auth/src/store/email_ownership.rs).

## What

Internal ownership preparation, Apple account selection and subject-indexed deletion for proven
emails. Owners and indexes live in the existing AuthTable; no public linking API is exposed.

## Why

Pending requests must not claim an address. Verified credentials and ownership must commit together,
and deletion must remove the subject association without permitting takeover or losing retry state.

## How

`email:owner` is keyed by the existing normalized-email `LookupFamily::Email` HMAC digest.
`email_by_subject:{subject}` uses the same digest as sort key. The reverse-index value contains
only digest and subject. Neither record contains raw email, password hashes, bearer values or
provider profiles, and owners, indexes and tombstones have no DynamoDB TTL.

An active owner has `subject: Some`, no deleted_at and no reusable_after. A deleted owner has
`subject: None`, a deletion timestamp, and optional reuse permission at/after that timestamp.
Typed readers reject unknown fields, wrong key/value bindings, blank subjects and invalid state
combinations with a sanitized storage error. Raw JSON is retained for exact-value conditions.

`prepare_email_ownership` writes nothing. Absence returns a NotExists condition; a valid reusable
tombstone returns an exact-value condition. Any active owner rejects, without inspecting or repairing
its account. `never` denies reuse; `immediate` still requires stored reusable_after permission;
`after_retention` additionally requires that deadline to be reached using the supplied registration
clock. Missing permission cannot be overridden by selecting immediate reuse. Policy denial is not
an operational failure. Registration includes both owner/index writes in its eight-operation commit.

`prepare_email_account` resolves an email digest supplied by validated Apple authority. An active
owner requires its matching active account and exact email-by-subject index. Missing/inactive
accounts reject; invalid owner/index shapes or bindings are storage errors, never an available
address. Otherwise absence or an eligible tombstone prepares a fresh generation-zero account.
The private `EmailAccountPlan` captures raw owner/index values or the exact creation condition;
provider issuance adds those two guarded operations to its account/identity/index/code transaction.
It does not reread or repair records at commit. The same reuse predicate serves registration and
Apple selection, with the caller's controlled clock. Preparation performs no writes.

`tombstone_email_owners` runs after the existing identity/password/reset/refresh cleanup in
`delete_account`. It queries only the requested subject's index partition; the production adapter
follows all query pages with consistent reads. After existing TTL filtering, malformed physical
keys/values/JSON fail closed in the adapter rather than hiding unfinished cleanup. Each index's key,
digest and subject are validated, then its exact owner is read. For an active owner still pointing
to that subject, one transaction replaces the complete owner with a subjectless tombstone and
deletes the exact index value.
Deletion/reuse timing comes from the account's persisted deletion timestamp and existing policy.

An absent owner, existing tombstone or different subject's owner is not rewritten. Removing its
stale index still requires an owner NotExists or exact-value ConditionCheck alongside exact index
deletion. A competing change rejects the transaction instead of removing a newer association.
An old subject's deletion retry cannot tombstone a newer owner or remove its new subject index.

Cleanup errors propagate to the existing delete caller. Its next attempt can finish indexed work
after confirmed failure or a lost response; no new receipt or compensation is introduced. Earlier
account/credential cleanup remains committed, so deletion is not atomic as a whole. Partial cleanup
may block registration until retry. Disable, enable and password reset leave ownership intact.

Password completion and new Apple identities claim ownership atomically. Active Apple identities
resolve before email; changed claims do not transfer or add ownership. Google does not consult this
index. Pending password requests do not reserve email, and an active owner still blocks password
completion rather than enrolling a password. This is not account merging or a general uniqueness
rule across providers. There is no alias rewriting, compatibility path, migration or backfill.
