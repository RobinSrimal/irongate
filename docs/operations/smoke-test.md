# Smoke Test

## Goal

Run a basic smoke test against a deployed Irongate stage.

## Inputs Needed

- `ApiUrl` from SST outputs.
- A configured OAuth client.
- Access to the Resend inbox or destination email.

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
- Password registration sends a Resend verification email.
- Verification consumes the token once.
- Login returns an authorization code redirect.
- `/token`, `/userinfo`, refresh, and `/oauth/revoke` succeed for a verified user.

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
