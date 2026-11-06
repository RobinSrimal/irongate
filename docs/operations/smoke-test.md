# Smoke Test

## Goal

Run a basic smoke test against an explicitly authorized deployed Irongate stage. This guide does
not authorize deployment, resource reconfiguration or data cleanup.

## Inputs Needed

- `ApiUrl` from SST outputs.
- A configured OAuth client.
- Access to the Resend inbox or destination email.

## Registration Preconditions

Use a disposable address and the [current email-first contract](../setup/06-resend-email.md).
Pending state must have a bound `registration_id` and no installed hash. Unbound proofs are invalid
grants. The template uses one current contract without compatibility branches, migration or backfill;
this guide does not authorize data cleanup.

## Commands

Discovery:

```bash
curl "$ApiUrl/.well-known/openid-configuration"
curl "$ApiUrl/.well-known/jwks.json"
```

DynamoDB TTL:

```bash
aws dynamodb describe-time-to-live --table-name "<TableName>" --profile "<profile>"
```

Admin unsigned rejection:

```bash
curl -i "$ApiUrl/_admin/users/user_smoke"
```

## Validation

- Discovery returns the configured issuer and mounted endpoints.
- JWKS returns public key material only.
- Admin unsigned request returns `403`.
- `POST /password/register` with only `{email}` returns `verification_required` and sends a Resend
  setup email, with no subject, OAuth code, tokens or login session.
- `POST /password/verify` with `{token, new_password}` returns `verified` and a subject after
  completion, without tokens or a session. Email/password registration, token-only verification
  and unknown fields are rejected.
- Login returns an authorization code redirect.
- `/token`, `/userinfo`, refresh, and `/oauth/revoke` succeed for a verified user.

## Registration Checks

For an authorized disposable registration check, request two links for the same pending instance.
In the existing optional web example, opening, refreshing or issuing HEAD to `/auth/verify-email`
must not call auth, consume a proof or create an account/session. A password/confirmation form
renders on GET; only deliberate POST completes setup. Missing/foreign/null/malformed Origin and
mismatched confirmation must reject before forwarding. Core CORS controls browser response access,
not admission; explicit Origin enforcement here belongs to the web registration routes.

Submit a weak password and confirm rejection leaves the proof usable, then complete using a valid
password. Sign in separately with the chosen password. Replay the used link and submit the sibling
link with a different password: both must fail without replacing the credential. Expired,
wrong-purpose or mismatched proofs also fail; controlled malformed-state and expiry cases belong
in local tests, not hand-edited deployed records.

Completion prepares proof/binding/identity/email ownership eligibility before hashing, then commits
proof deletion, verified credential, account, identity, email owner and three indexes in one
eight-operation transaction. Pending/resend state claims no ownership. An active owner blocks
completion without linking accounts, even if its account is disabled, deleted or missing; such
controlled inconsistent-state cases belong in local fixtures. A smoke test does not establish race
atomicity. Domain tests cover policy/proof rejection, sibling/replay
and delivery failure; storage/race verification uses the
[local test guide](../../packages/functions/auth/tests/README.md), not destructive deployed-table tests.
No test pass is implied by these instructions.

A delivery failure can leave current-contract pending state and an unused proof; another request
can send a fresh link for that eligible instance. Do not force a live delivery outage to test it.
A lost completion response can follow commit, so an error is not evidence of rollback. Do not
copy bearer links, tokens or passwords into notes. No hosted core pages or new infrastructure are
needed for this flow.

## Provider Checks

For an explicitly authorized provider check, use a disposable Google or Apple identity with the
existing [Google](../setup/07-google-login.md) or [Apple](../setup/08-apple-login.md) configuration.
No new settings, routes, hosting or infrastructure are required.

For Google, start through browser navigation to the HTTPS issuer in the same browser that receives
the callback. Verify a scoped `__Host-ig-google-<state-HMAC>` cookie is set on provider start with
Secure, HttpOnly, SameSite=Lax, Path=/, no Domain and bounded Max-Age. Confirm no-store/no-referrer
on start/callback and cookie clearing after confirmed consumption. Do not copy cookie values,
state digests or callback URLs into logs or notes. Old/unbound transactions need a fresh sign-in.
Missing/duplicate/wrong-cookie and concurrent-consumer cases belong in the controlled local HTTP
and storage suites; their passing results do not certify actual browser enforcement or Google
service acceptance. Apple does not use this Google-cookie contract.

Confirm the validated callback returns to the exact registered client redirect with a code and
original state, not access/refresh/ID tokens. Exchange the code through the normal client flow and
repeat sign-in to confirm a stable subject. A new Apple identity with signed verified email, or
Google identity with verified Gmail/valid signed hosted-domain authority, should share an active
owner's subject for the same address. New Google identities without that authority reject in public
callbacks; store-only confirmation has no public route or delivery integration. Its proof/transaction
contracts run locally, not through this deployed smoke flow. A private-relay address is distinct from the hidden real
address. Existing accounts must not merge. Provider-first then password enrollment is not supported: pending password completion must
not install a credential on that account. Changed/missing signed email and unsigned Apple profile
rejection belong in controlled local fixtures, not live provider-profile manipulation.

Provider preparation pins the exact identity/account generation without writes and checks the
consumed session's selected provider and expiry. Finish rechecks expiry before one transaction:
six operations for a new/reusable Google or Apple identity including account/email-owner/index
guards or creation alongside identity/index/code; or three operations for a
returning identity. A smoke test does not prove
race atomicity, TTL consistency or uncertain-outcome handling; use the
[local test guide](../../packages/functions/auth/tests/README.md) with disposable tables for those
contracts. Do not induce outages, conflicting writes or malformed records in a deployed table.

Confirmed conditional/stale issuance or blocked reuse returns fixed `400 invalid_grant`
(`provider sign-in is no longer valid`). Operational storage failures, transaction conflicts and
uncertain outcomes return fixed `500 server_error` (`provider sign-in unavailable`), not invalid
credentials. State/session consumption occurred before exchange/validation and is not rolled back;
restart sign-in instead of replaying the callback. There is one application-level issuance attempt,
without re-resolution or upstream exchange retry. A lost response can follow a complete commit:
do not compensate by deleting an account; unreturned codes expire normally.

Exact-value conditions check state at commit, not disable/enable history that restores the same
value. If issuance wins a race, later token exchange still checks account status/generation; existing
JWTs keep their independent expiry. No instantaneous revocation or general cross-provider linking
guarantee is implied. Record observed outcomes without bearer values; this guide is not test evidence.

## Recovery Checks

For an explicitly authorized recovery check, use a disposable test account. Keep an unused code,
refresh token and second reset link, then complete one reset. Verify old-password login and the
older proofs/code/refresh fail, while new-password login and fresh refresh work. Merely requesting
the reset must not invalidate credentials. Do not print or save these bearer values in notes.
Already-issued access JWTs may remain valid until expiry; a response lost after a storage commit
does not imply rollback. Local deterministic transaction/race coverage is documented in
[the auth test guide](../../packages/functions/auth/tests/README.md).

## Deletion Checks

Only with explicit authorization, delete a disposable account through the existing SigV4 admin
route. Ownership cleanup runs after account marking, identity/password tombstoning, reset cleanup
and refresh-family revocation. For each indexed email it conditionally replaces the matching active
owner with a subjectless tombstone and deletes the exact reverse index in one transaction. The
tombstone keeps digest/retention metadata, not a raw email, subject link or TTL. Disable, enable and
password reset do not release ownership. Re-registration still requires both password identity and
email owner reuse policy to permit it, and creates a fresh subject rather than linking accounts.

Deletion is not one atomic transaction. A failure may leave the account marked deleted with earlier
cleanup committed and ownership still active. Errors remain visible; after resolving the failure,
retry the existing delete operation to finish cleanup. Do not treat an error as rollback or edit
deployed rows to force success. Malformed non-expired physical items and malformed typed ownership
records fail closed. Their controlled failure/repair cases, stale-index races and raw no-TTL checks
belong in the disposable Local suite, not a deployed smoke test. Admin responses do not expose
ownership digests or records.

## Audit And Logging

After an explicitly authorized deployment, inspect both existing Lambda log groups and execution
roles. Confirm configured finite retention (30 days by default), JSON/INFO application settings and
the normal SST logging permissions. API Gateway access logging is separate.

Using a disposable account and configured client, complete code exchange, refresh, logout and an
authorized SigV4 admin read. In the corresponding function groups, check for JSON
`record_type=auth_audit`, schema version 1, UUID, timestamp, INFO level and the expected fixed
event kind. Admin read must attribute the trusted IAM actor separately from the target subject.
Use a bounded time range and do not copy event identifiers or bearer values into public notes.

Check for `record_type=audit_emit_failed` (ERROR) to identify local validation/serialization/size/
output failures. Diagnostics contain only safe metadata/category, not the rejected record or error
text. Absence is not proof of delivery: total output failure may report nothing. Do not induce a
production output outage to test the fallback; local tests cover it.

Only with separate reconfiguration approval, `AUTH_AUDIT_LOG_MODE=none` can be checked in a test
deployment: no audit events/diagnostics should be emitted, while operational/access logs continue.
Restore the approved stage setting afterwards. Existing DynamoDB audit rows are untouched and do
not expire under CloudWatch retention. No table scan or cleanup is required for this smoke check.

Record observed pass/fail/blocked outcomes and the stage/time without secrets. This guide itself
is not evidence that CloudWatch delivery, retention or IAM were tested.

## Reference

The first dev deployment validation is recorded in:

```text
design/research/aws-dev-smoke-test-2026-06-16.md
```
