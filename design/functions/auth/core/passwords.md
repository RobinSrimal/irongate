# Password Domain

Implementation: [policy primitives](../src/core/passwords.md),
[password operations](../src/providers/password.md) and [HTTP handlers](../src/api/providers/password.md).

## What And Why

Registration, verification and authentication are separate operations. Creating an unverified
password user or verifying an email does not issue an OAuth code. Login requires a correct password,
a verified persisted subject and an active account.

## How

Registration normalizes email, hashes it for lookup, stores an Argon2id password hash and sends a
verification link. Verification consumes its one-time proof and resolves the password account.
Login verifies the stored hash, checks account status, consumes an authorize session and creates
a generation-bound authorization code with account and exact verified-user conditions.

Forgot-password guards eligibility and generation while creating a short-lived proof/index, then
sends email. It does not invalidate existing credentials. Reset prepares validated snapshots without
consumption, hashes the new password and commits proof/index deletion, password replacement and
account generation increment in one transaction. Expiry is checked again before dispatch.

Older reset links, codes and refresh tokens for that subject fail generation checks without a scan.
Confirmed condition rejection leaves no partial recovery commit; operational or uncertain failures
do not prove rollback. Recovery never reactivates an account or automatically logs in. Registration
and email verification retain their separate proof/account steps. Access JWTs expire independently.

Policy and hashing details are in [password hashing](../crypto/passwords.md). There is no call-site
flag allowing login to skip verification, and no automatic successful-login rehash.
