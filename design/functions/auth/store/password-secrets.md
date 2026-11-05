# Password Verification And Reset Secrets

Implementation: [verification and cleanup](../src/store/password_secrets.md) and
[atomic registration](../src/store/registration.md), plus [atomic recovery](../src/store/recovery.md).

## Owns

- Email verification link-token records.
- Password reset link-token records.
- HMAC lookup for verification and reset secrets.

## Behavior

Email-only registration creates/reuses an unverified pending instance without a hash, subject or
ownership, then creates a verification proof bound to its `registration_id`. Forgot-password creates
a reset proof for an eligible active account.

Verification and reset use high-entropy link tokens.

The raw link token is sent by email once as part of a verification or reset URL. DynamoDB stores only a lookup digest:

```text
verification_lookup_digest = hex(HMAC-SHA256(storage_lookup_secret, "email_verification" || NUL || secret))
reset_lookup_digest = hex(HMAC-SHA256(storage_lookup_secret, "password_reset" || NUL || secret))
```

Verification record shape:

```json
{
  "email_digest": "...",
  "registration_id": "<pending-instance-uuid>",
  "purpose": "verify_email",
  "created_at": "...",
  "expires_at": "..."
}
```

Reset record shape:

```json
{
  "email_digest": "...",
  "subject": "user_...",
  "credential_generation": 0,
  "purpose": "reset_password",
  "created_at": "...",
  "expires_at": "..."
}
```

Email verification expiry is derived from `AUTH_EMAIL_VERIFICATION_TTL_SECONDS`. Password reset expiry is derived from `AUTH_PASSWORD_RESET_TTL_SECONDS`. Both are written inside the record and as the DynamoDB `expiry` attribute.

## Store Operations

```text
create_email_verification
prepare_password_registration
finish_password_registration
create_password_reset
prepare_password_reset
finish_password_reset
delete_password_secrets_for_subject
```

## Security Invariants

- Raw verification and reset secrets never appear in `pk`, `sk`, logs, or errors.
- Verification and reset link tokens are high entropy.
- Verification and reset link tokens are short-lived.
- Verification and reset secrets are single-use.
- Expired records are rejected even if DynamoDB TTL has not deleted them.
- Verification consumption and initial credential/account/identity/email-owner/index creation
  share one conditional commit.
- Recovery commits proof/index consumption, password replacement and generation increment together.
- Account disable/delete paths can remove reset secrets for the subject.

## Consumption Boundary

Link tokens are created and consumed through purpose-specific store operations, not generic writes.
Verification issuance checks the complete eligible pending value and creates the proof with matching
logical/physical expiry in one transaction. Preparation checks purpose, expiry, registration ID,
pending eligibility, email digest, identity reuse and email-owner eligibility without consuming.
Only then does the domain hash the chosen password. Finish rechecks expiry and commits eight
conditional operations for proof, credential, account, identity, email owner and three subject
indexes.
An active owner denies completion without proof consumption or enrollment onto another account.
Identity and owner tombstones must both permit reuse; malformed owner records remain storage errors.
Owner/index writes have no raw email or TTL and do not occur at proof issuance or resend.

Resends can leave sibling proofs. Once one completes, pending eligibility is gone, so all siblings
fail logically without a scan; a new registration instance cannot use an old proof. TTL eventually
cleans them up. Delivery failure does not delete pending state/proofs; a new request can send another
link. Old unbound pending records/proofs are not migrated or backfilled. Existing login/recovery is
unchanged, but completed accounts do not gain ownership records implicitly or through a backfill.

Reset creation guards active account and eligible password-user snapshots; preparation checks
purpose, expiry, subject binding and generation without consuming anything. Finish rechecks expiry
after hashing and uses complete previous-value conditions in one four-operation transaction.
Old links are invalidated by generation,
not partition-wide deletion. Confirmed rejection has no partial recovery writes; an operational
failure or lost response does not prove rollback. The same distinction applies to registration.
