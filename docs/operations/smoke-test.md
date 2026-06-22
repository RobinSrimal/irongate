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

## Reference

The first dev deployment validation is recorded in:

```text
design/infra/auth/aws-dev-smoke-test.md
```
