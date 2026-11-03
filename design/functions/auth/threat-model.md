# Auth Threat Model

This is the threat model for the public auth function and shared auth runtime.

## Assets

- Password hashes.
- Email verification and reset secrets.
- Authorization codes.
- Refresh tokens and token-family state.
- JWT signing keys.
- Provider credentials for Google, Apple, and Resend.
- HMAC lookup secret.
- OAuth client secrets.
- DynamoDB auth records.

## Actors

- Anonymous internet user.
- Legitimate end user.
- OAuth client application.
- External identity provider.
- Resend.
- Runtime Lambda role.
- Deploy/operator role.
- Break-glass operator.
- Attacker with raw AuthTable read access.
- Attacker with stolen browser redirect/code values.

## Trust Boundaries

- Browser/client to API Gateway.
- API Gateway to Lambda event/request context.
- Lambda to DynamoDB.
- Lambda to Resend.
- Lambda to Google/Apple.
- Lambda to KMS/secrets.
- AuthTable to any operator tooling.

## In Scope Abuse Cases

- Public route abuse.
- Registration before email verification.
- Password brute force and credential stuffing.
- Password reset replay.
- Authorization code replay.
- Refresh token replay and race conditions.
- Spoofed forwarded headers for rate-limit bypass.
- Raw table read exposure.
- Provider callback CSRF/state replay.
- OIDC token validation mistakes.

## Security Principles

- Keep the public auth Lambda free of runtime control-plane bootstrap.
- Use typed store operations instead of generic persistence primitives.
- Store lookup digests for bearer secrets, not raw values.
- Keep JWT private keys out of ordinary AuthTable reads.
- Use Resend for real verification/reset delivery in every stage.
- Use API Gateway request context for source IP, not forwarded headers.
- Keep raw auth state behind runtime and audited break-glass access only.

## Current Coverage Limits

These principles describe the intended protection of the listed assets, not proof that every abuse
case is closed. Strongly consistent reads and classified transaction failures in
[storage](store/dynamodb.md) do not make read-then-write workflows atomic. Refresh-family expiry is
renewed with its record replacement, but this does not couple account checks or JWT signing to
rotation. Current limits include separate proof-consume/password-replacement
writes in [recovery](core/passwords.md), composite rather than independent [rate limits](store/rate-limits.md),
and selected [audit records](observability/audit.md) persisted without TTL.

The [test overview](testing.md) distinguishes memory-backed coverage from deployed or local-DynamoDB
evidence. The [optional examples](../../examples/README.md) have their own session and platform limits.
