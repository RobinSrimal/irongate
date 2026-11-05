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
Old pending rows missing `registration_id` or containing a hash cannot be reused by registration;
requesting a fresh link alone does not repair them. Old unbound proofs are invalid grants, not
supported completion paths. Use fresh development state or separately reviewed cleanup before
deploying this contract over old pending state. No automatic erasure, migration or backfill is
provided or required by the clean contract. Completed users and credentials remain unaffected.

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

Completion prepares proof/binding/identity eligibility before hashing, then commits proof deletion,
verified credential, account, identity and two indexes in one six-operation transaction. A smoke
test does not establish race atomicity. Domain tests cover policy/proof rejection, sibling/replay
and delivery failure; storage/race verification uses the
[local test guide](../../packages/functions/auth/tests/README.md), not destructive deployed-table tests.
No test pass is implied by these instructions.

A delivery failure can leave current-contract pending state and an unused proof; another request
can send a fresh link for that eligible instance. Do not force a live delivery outage to test it.
A lost completion response can follow commit, so an error is not evidence of rollback. Do not
copy bearer links, tokens or passwords into notes. No hosted core pages or new infrastructure are
needed for this flow.

## Recovery Checks

For an explicitly authorized recovery check, use a disposable test account. Keep an unused code,
refresh token and second reset link, then complete one reset. Verify old-password login and the
older proofs/code/refresh fail, while new-password login and fresh refresh work. Merely requesting
the reset must not invalidate credentials. Do not print or save these bearer values in notes.
Already-issued access JWTs may remain valid until expiry; a response lost after a storage commit
does not imply rollback. Local deterministic transaction/race coverage is documented in
[the auth test guide](../../packages/functions/auth/tests/README.md).

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
