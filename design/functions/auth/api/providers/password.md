# Password Provider API

Implementation: [password HTTP handlers](../../src/api/providers/password.md).

## Owns

- Password registration endpoint.
- Password login endpoint.
- Email verification endpoint.
- Password reset request and reset completion endpoints.
- Delegating registration/completion and email delivery to the password domain.

## Registration Contract

```text
POST /password/register
{ "email": "user@example.com" }
-> { "status": "verification_required" }

POST /password/verify
{ "token": "<email-proof>", "new_password": "<chosen-password>" }
-> { "status": "verified", "subject": "user_..." }
```

Both JSON request types reject unknown fields. Password-first registration and token-only
verification are invalid requests, not compatibility paths. Registration returns no subject or
delivery ID; completion returns a subject but no login session or tokens. Login remains a separate
form-encoded OAuth continuation with an authorize session.

## Flow

```text
register email only
  -> create/reuse pending registration_id with no hash, subject or email ownership
  -> conditionally create verification proof bound to that instance
  -> send verification email
  -> return verification_required
  -> no OAuth authorization code

verify email with token + new_password
  -> validate password policy
  -> prepare proof/pending user/identity/email-owner eligibility without consuming
  -> hash password and recheck expiry
  -> atomically consume proof and write verified user/account/identity/email-owner/three indexes
  -> return verified + subject, no OAuth code or tokens

login email + password
  -> verify password hash
  -> require verified email
  -> require active account
  -> issue OAuth authorization code

forgot password
  -> conditionally create reset link token bound to active account generation
  -> send reset email

reset password
  -> prepare validated reset proof/account/user without consuming
  -> hash new password and recheck expiry
  -> atomically consume proof/index, replace password and increment account generation
```

## Security Invariants

- Registration and completion never issue tokens or establish a login session.
- Registration responses must not include an OAuth code, access token, refresh token, or authenticated subject.
- Password policy is enforced before creating or updating a hash; initial hashing follows proof preparation.
- Passwords are stored only as Argon2id hashes.
- Verification and reset link tokens are high-entropy, short-lived, and single-use.
- Verification and reset lookups use HMAC digests, not raw link tokens in keys.
- Registration, login and forgot-password use independent normalized-email and trusted-source limits;
  verification and reset completion use source limits before protected work, never raw-token keys.
- Disabled or deleted accounts cannot receive an OAuth authorization code.
- Registration retains the explicit already-registered response; it does not promise enumeration resistance.
- Configured `allowed_origins` controls browser access to CORS responses, not core request admission.
  It does not prevent a request or its effects; the web example's registration/completion form routes
  separately enforce explicit same-origin checks before forwarding.
- CORS responses must not use wildcard origins.

Invalid, expired, wrong-purpose, mismatched, consumed or sibling proofs after completion and confirmed
completion rejection use the same safe `invalid_grant`. Policy failure is `invalid_request` and
leaves the proof usable. Registration/verification storage failures use fixed `server_error`
messages; delivery failure uses `temporarily_unavailable`. Neither an operational error nor a lost
response proves rollback. A failed email send may leave pending state and an unused proof.

An active email owner blocks completion without consuming the proof, even if its account is
disabled, deleted or missing. This is not password enrollment onto an existing subject or provider
linking. Ineligible tombstones are also safe completion rejection; malformed ownership records are
storage failures. The owner and its reverse index commit only within eight-operation completion,
are persistent with no raw email or TTL, and are not exposed by these responses.

Proofs with absent, empty or non-string `registration_id` are invalid proofs: the typed store checks
the raw binding before decoding, returning the same `invalid_grant` without a default/fallback ID.

The existing [web example](../../../../examples/web/src/routes.md) owns a non-mutating verification
GET/HEAD and deliberate same-origin POST with password confirmation. The core stays API-only; no
core-hosted pages or new infrastructure are required. Old pending rows missing an ID or containing
a hash cannot be reused by registration; requesting a fresh link alone cannot fix them. The clean
contract requires fresh development state or separately reviewed cleanup before deployment, not
automatic erasure, migration or backfill. Old unbound proofs cannot complete registration; completed
users retain their existing login/recovery without ownership lookups. Historical accounts are not
automatically indexed, and there is no ownership backfill. Fresh disposable acceptance data or a
separately approved data-transition decision is required before relying on complete coverage.
See [email setup](../../../../../docs/setup/06-resend-email.md).

Confirmed reset-proof or commit rejection returns a generic invalid-reset error. Policy errors stay
distinct; operational/uncertain failures stay server errors. Recovery invalidates older codes and
refresh credentials without changing existing access-JWT expiry or returning tokens.
