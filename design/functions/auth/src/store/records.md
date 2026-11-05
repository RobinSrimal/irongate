# Auth Records

Source: [packages/functions/auth/src/store/records.rs](../../../../../packages/functions/auth/src/store/records.rs).

## What

Serializable account, identity, email-owner, password, proof, session, code and refresh records.

## Why

Typed records preserve the information needed for expiry, replay and lifecycle checks.

## How

Short-lived records carry `expires_at` independently of physical expiry. Account/identity statuses
encode lifecycle state; optional password/contact fields support pending users and tombstones.

`EmailOwnerRecord` contains email digest, optional subject, deleted_at and reusable_after.
An active owner has a nonblank subject and neither deletion nor reuse timestamp. A tombstone has
no subject, a deletion timestamp, and either no reuse permission or a reuse timestamp at/after
deletion. `EmailSubjectIndexRecord` contains only email digest and subject. Both reject unknown
fields. [Ownership helpers](email_ownership.md) validate key/value bindings and state combinations
on reads, preserve raw JSON for exact-value conditions and return a sanitized storage error for
malformed records. The records contain no raw email, credentials or profile data and have no TTL.

Password completion and new Google/Apple identity issuance write ownership with email authority;
pending registrations, Google callbacks and returning provider identities do not claim it.
Deletion clears the owner's subject and removes its reverse index. Existing password accounts
without these rows remain eligible for their existing login/recovery flows; no read performs a
lazy backfill or certifies complete historical ownership coverage.

`PasswordUserRecord.registration_id` is optional: pending users require a nonempty ID, normalized
email, `verified=false` and no subject, hash, hash timestamp or deletion marker. Completed users
have a subject/hash/hash timestamp and `verified=true`, with no registration ID. Tombstones clear
the binding and credential/contact fields. Absent optional IDs are omitted when serialized.
`EmailVerificationRecord` requires `registration_id` alongside email digest, purpose and timestamps;
the ID binds one pending instance, not a bearer credential or a lookup key.

Old pending records without a valid binding or with a hash are ineligible for reuse or completion.
Although the typed proof writer requires a string ID, preparation checks the raw JSON binding before
decoding: missing, empty or non-string proof IDs return invalid proof (`invalid_grant`), not a
deserialization server error. There is no generated default, timestamp fallback, migration or
backfill. Existing completed password users need no rewrite; an absent pending ID is their normal state.

Accounts, reset proofs, authorization codes, refresh tokens and refresh families require
`credential_generation: u64`.
Missing fields fail deserialization, not default to zero. Recovery increments the account counter;
bound credentials retain their original generation. It is not a JWT claim or an admin response field.
