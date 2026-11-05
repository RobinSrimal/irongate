# Password Domain

Source: [packages/functions/auth/src/providers/password.rs](../../../../../packages/functions/auth/src/providers/password.rs).

## What

Registration, verification, login, reset request and reset completion operations.

## Why

Only the email-proof holder can choose the initial password. Registration and completion are
separate from password login and OAuth continuation.

## How

Registration accepts email only, normalizes it and uses its HMAC lookup digest to create or reuse an
eligible pending user. No hash, hash timestamp, subject, account or email ownership is installed.
Resends retain the pending `registration_id` and creation time; deleted-user recreation requires
the existing identity reuse policy and creates a fresh binding. A conditional create/recreate race
rereads once and accepts only an eligible pending winner for the same email.

A high-entropy token is persisted as a digest-keyed proof bound to that pending instance before
email delivery. Proof issuance guards the exact pending snapshot. Delivery failure leaves persisted
pending state/proof in place; another rate-limited registration can request a fresh link.

Verification requires `token` and `new_password`. It validates password policy, prepares the proof,
pending user, identity and email-owner eligibility without consumption, then hashes and calls
[atomic registration completion](../store/registration.md). The eight-operation finish rechecks expiry,
consumes the proof and creates the verified credential, account, identity, email owner and indexes.
Active owners deny completion without linking or account repair; eligible identity and owner
tombstones must both satisfy reuse policy. Malformed ownership records are storage failures, not
available addresses. Invalid proofs, replay, sibling links after completion and confirmed condition
rejection cannot change a password.
Policy failure leaves the proof usable; operational/uncertain failures do not imply rollback.
Neither registration nor completion issues tokens or establishes a login session.

Ownership uses only digest/subject/deletion metadata, with no raw email or TTL in those records.
It is not inferred for historical accounts or provider identities. Existing login/recovery performs
no new ownership lookup or lazy backfill. Ownership acceptance requires fresh disposable data.

Login checks hash, verified user and active account, consumes a session, then creates a
generation-bound code guarded by both account and verified user snapshots. A stale password check
cannot acquire the new generation after recovery.

Forgot-password returns a generic response; eligible proof creation guards account/user snapshots
before mail is sent. Confirmed condition races return that response without sending mail; operational
failures propagate. Completion validates policy, prepares proof/account/user without consumption,
hashes and finishes one conditional recovery transaction. Confirmed rejection is generic invalid
reset; uncertain or operational failures are not reported as success or rollback. No reset response
issues tokens or logs in the user.
