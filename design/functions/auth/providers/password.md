# Password Identity Provider

Implementation: [password domain](../src/providers/password.md).

## Owns

- Password registration.
- Password hash verification.
- Email verification state.
- Password reset state.
- Mapping verified password users to internal identity.

## Behavior

Password login verifies that the caller knows the password for a registered and verified email address.

Registration accepts email only and creates or reuses a pending instance with a fresh-on-creation
`registration_id`, no password hash, subject or email ownership. Resends retain the instance and
send a new high-entropy proof bound to it. Issuance checks the exact pending snapshot before email
delivery.
Delivery failure may leave pending state and an unused proof; retrying registration can send a
fresh link without replacing the instance.

Completion requires the email proof and a new password. Policy validation precedes non-consuming
preparation; hashing occurs only after proof, pending binding, password-identity and email-owner
eligibility checks succeed. [Atomic registration](../src/store/registration.md) consumes the proof
and writes the verified credential, fresh generation-zero account, identity, email owner and three
subject indexes in eight conditional operations. Active ownership denies completion regardless of
its account's status, without attaching a password or linking accounts. Both owner and identity
tombstones must permit reuse. Ownership values contain no raw email or TTL.
Replay and sibling links cannot replace the completed password. Deleted-identity reuse obeys
configured policy and always creates a new binding and subject, never a revived account.

Registration and completion are not login. They issue no OAuth code or auth tokens and create no
login session. After completion, separate login uses the generated persisted subject and must fail
if its account is disabled or deleted. This flow does not perform cross-provider account linking.

Login binds its code to the active account generation and guards the verified password snapshot.
Forgot-password creates a generation-bound proof before sending email; a confirmed eligibility
race produces the same generic response without mail. Reset prepares without consumption, hashes,
then atomically consumes proof/index, replaces the password and increments generation. Older reset
proofs, codes and refresh sessions for that subject become unusable; no tokens are returned.
An operational failure remains an error, not a claim of rollback or successful delivery.

## Security Invariants

- Passwords are hashed with Argon2id.
- Passwords must be 12 to 128 characters.
- Password policy does not require character composition rules.
- Verification and reset link tokens are high-entropy and single-use.
- Verification and reset link tokens expire quickly.
- Login, registration, verification, and reset attempts are rate-limited.
- Normalized email is used consistently.
- No password is accepted or hashed at registration before proof validation.
- Verification completion consumes its proof and activates credentials in one conditional commit.
- Policy errors leave proofs usable; invalid proofs and confirmed commit rejection do not activate users.
- Email verification is required before password login can issue a code.
- Email delivery failures do not mark users verified.
- Disabled or deleted accounts cannot log in.

The [API contract](../api/providers/password.md) rejects password-first registration and token-only
verification. Old pending records/proofs are invalidated without migration/backfill; completed
password login/recovery is unchanged. Historical ownership coverage is not backfilled or inferred.
Use fresh disposable acceptance data; complete coverage for an existing deployment requires a
separate data-transition decision. Active registration still reports already-registered email;
enumeration resistance is not claimed.
