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
an authorization code.

Forgot-password sends a short-lived reset proof for an eligible account. Reset consumes that proof
and then replaces the stored password hash. Proof consumption and password replacement are separate
writes; the current flow is not an atomic recovery transaction and has no credential-generation
counter invalidating every older credential.

Policy and hashing details are in [password hashing](../crypto/passwords.md). There is no call-site
flag allowing login to skip verification, and no automatic successful-login rehash.
