# Atomic Password Registration

Source: [packages/functions/auth/src/store/registration.rs](../../../../../packages/functions/auth/src/store/registration.rs).

## What

Preparation and atomic completion of a password registration using an email proof and chosen hash.

## Why

Proof consumption must commit with the verified credential, account and indexes. Replay, sibling
proofs or a stale pending snapshot must not install a second password or leave partial activation.

## How

`prepare_password_registration` reads the proof as a raw JSON value inside the typed store first.
An absent, empty or non-string `registration_id` returns `None` before typed decoding, including
old unbound proofs. The domain maps this to invalid proof and the API to `invalid_grant`, not a
deserialization server error. There is no default ID or fallback. The typed writer still requires
a string binding; other malformed typed fields remain storage errors.

Preparation then checks proof purpose and logical expiry, an eligible pending user
with the same `registration_id`, and the normalized email's HMAC binding. It requires an absent
password identity or an eligible deleted identity under the existing reuse policy. It returns an
opaque `PreparedPasswordRegistration` with private snapshots/conditions without consuming anything.
The [domain](../providers/password.md) validates policy before preparation and hashes only afterward.

`finish_password_registration` consumes that prepared value and a hash, generates a fresh subject,
revalidates pending eligibility/binding and rechecks expiry with the injectable registration clock.
It builds one six-operation storage transaction:

1. Delete the proof conditioned on its complete prepared value.
2. Replace the exact pending user with a verified credential, setting subject/hash/hash timestamp
   and clearing `registration_id`.
3. Create an active generation-zero account conditioned on absence.
4. Create the password identity conditioned on absence, or replace the exact eligible tombstone.
5. Create the identity-to-subject index conditioned on absence.
6. Create the password-user-to-subject index conditioned on absence.

Expiry is checked again immediately before dispatch. Conditional rejection returns `None`; a
confirmed canceled transaction has no partial writes. Operational errors propagate, and a lost
response after commit is not proof of rollback. Application expiry checks do not imply a zero-time
network/storage commit; DynamoDB TTL is only eventual cleanup.

Exactly one competing completion can replace the pending snapshot. A sibling proof may remain
physically stored, but cannot pass pending eligibility after success. A recreated registration has
a different ID, so an earlier instance's proofs cannot activate it. Completion never returns success
merely because a user is already verified, reuses an old subject, enables an account, or links
provider accounts. There is no standalone consume/activate path or legacy pending-data backfill.
