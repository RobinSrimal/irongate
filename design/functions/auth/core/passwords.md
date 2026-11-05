# Password Domain

Implementation: [policy primitives](../src/core/passwords.md),
[password operations](../src/providers/password.md) and [HTTP handlers](../src/api/providers/password.md).

## What And Why

Registration, verification and authentication are separate operations. Creating an unverified
password user or verifying an email does not issue an OAuth code. Login requires a correct password,
a verified persisted subject and an active account.

## How

Registration accepts email only, normalizes it, derives its HMAC lookup digest and stores/reuses a
pending `registration_id` with no hash, subject or email ownership. A conditionally issued proof
binds to that instance before email delivery. Resends preserve the pending binding; delivery failure
may leave pending state and an unused proof but cannot activate a credential.

Completion requires `token` and `new_password`. It validates the 12-to-128-character policy, prepares
the proof/pending user/identity/email-owner eligibility without consumption, then hashes with
Argon2id. A weak password leaves the proof usable; invalid proofs are rejected before hashing.
[Atomic registration](../src/store/registration.md)
rechecks expiry and commits proof deletion, verified credential, fresh generation-zero account,
password identity, email owner and three subject indexes in eight operations. Active owners cannot
be taken over or used to enroll a password onto another account; identity and owner tombstones
must both permit reuse. Replay or sibling proofs cannot change the password;
confirmed condition rejection has no partial activation. Neither endpoint logs in or issues tokens.

Ownership records contain only digest, subject and lifecycle metadata without TTL. Existing
login/recovery does not require them and does not backfill historical accounts. Provider callbacks
do not consult or claim ownership; matching emails still do not link accounts.

Login verifies the stored hash, checks account status, consumes an authorize session and creates
a generation-bound authorization code with account and exact verified-user conditions.

Forgot-password guards eligibility and generation while creating a short-lived proof/index, then
sends email. It does not invalidate existing credentials. Reset prepares validated snapshots without
consumption, hashes the new password and commits proof/index deletion, password replacement and
account generation increment in one transaction. Expiry is checked again before dispatch.

Older reset links, codes and refresh tokens for that subject fail generation checks without a scan.
Confirmed condition rejection leaves no partial recovery commit; operational or uncertain failures
do not prove rollback. Recovery never reactivates an account or automatically logs in. The same
error-outcome distinction applies to registration. Access JWTs expire independently.

Policy and hashing details are in [password hashing](../crypto/passwords.md). There is no call-site
flag allowing login to skip verification, and no automatic successful-login rehash.
