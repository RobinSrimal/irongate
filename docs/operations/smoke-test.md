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

## Reference

The first dev deployment validation is recorded in:

```text
design/research/aws-dev-smoke-test-2026-06-16.md
```
