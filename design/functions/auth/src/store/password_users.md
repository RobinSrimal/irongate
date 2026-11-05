# Password Users

Source: [packages/functions/auth/src/store/password_users.rs](../../../../../packages/functions/auth/src/store/password_users.rs).

## What

Pending password registration creation, lookup, deleted-record recreation and tombstones.

## Why

Pending state must not contain an anonymous caller's password or inherit an earlier registration
binding. Lifecycle rules belong in typed persistence rather than route-controlled writes.

## How

Creation writes a normalized email and fresh UUID `registration_id` under the email-digest key,
conditioned on absence. Pending eligibility requires a nonempty binding/email, no subject, hash,
hash timestamp or deletion marker, and `verified=false`. Creation/update timestamps use the
registration clock. Resends reuse eligible pending state without rewriting it.

Deleted-record recreation replaces the exact tombstone with fresh pending state; the domain checks
identity-reuse policy before calling it and [registration preparation](registration.md) checks the
identity again. There is no standalone activation or password-first creation helper. Completion
writes the hash, new subject and indexes only within the registration transaction.

Subject cleanup queries password-user indexes. It conditionally replaces matching users with
tombstones, clearing registration ID, email, subject, hash and hash timestamp, and deletes the index
in the same transaction. Missing users have their stale index removed. Password replacement is
available only through the [recovery transaction](recovery.md), not an independent hash-update API.
