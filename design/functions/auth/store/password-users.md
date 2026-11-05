# Password Users

Implementation: [store/password_users.rs](../src/store/password_users.md).

## What And Why

Password users are keyed by an HMAC of normalized email. A pending value contains the normalized
email, nonempty `registration_id`, `verified=false` and creation/update timestamps, but no subject,
hash, hash timestamp or deletion marker. Only completion stores a PHC hash, so an anonymous request
cannot choose the password that an email owner later activates. Raw passwords are never persisted.

## How

The module supports conditional pending creation/recreation, lookup and tombstoning. Password
verification and OAuth continuation are domain behavior in [providers/password](../src/providers/password.md),
not a generic store login method.

Creation requires absence and generates a new UUID binding. Resends retain an eligible existing
pending instance without changing its ID or creation time. Deleted recreation replaces an exact
tombstone only after domain reuse-policy checks and generates a fresh binding.

[Atomic registration](../src/store/registration.md) validates proof/binding/identity eligibility
without consuming, then commits proof deletion and the verified user, generation-zero account,
identity and subject indexes in one six-operation transaction. Completion clears `registration_id`
and installs the email-proof holder's chosen hash and a fresh subject. Sibling and replayed proofs
cannot change that credential. Login requires the verified subject and active account status.
Reset replaces the hash and timestamps only within [atomic recovery](../src/store/recovery.md),
together with proof/index consumption and account generation increment. There is no independent
hash-update operation. Password login code creation guards the exact user snapshot whose hash was
verified, preventing an old password check from gaining the new generation.
Account deletion replaces the password record with a tombstone that clears its registration ID,
email, subject, password hash and hash timestamp while retaining lifecycle timestamps. A recreated
pending instance cannot accept an earlier instance's proof or revive the deleted subject.

Old pending records without a binding or with a preinstalled hash are ineligible. There is no
migration/backfill or legacy activation path. Completed users without a registration ID remain
valid and need no rewrite.

There is no automatic login-time hash upgrade or current-password-change endpoint.
