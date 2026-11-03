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
- Password reset replay, partial recovery and stale credential writers.
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
- Commit reset proof consumption, password replacement and generation invalidation together.
- Guard code/refresh writes with the active account generation; guard password login with the
  exact verified password snapshot as well.

## Current Coverage Limits

These principles describe the intended protection of the listed assets, not proof that every abuse
case is closed. Strongly consistent reads and classified transaction failures in
[storage](store/dynamodb.md) do not alone make workflows atomic. [Recovery](core/passwords.md)
has a guarded transaction; codes/refreshes retain generation and account conditions. JWT signing and
delivery remain separate, so in-flight authorized requests can complete after recovery and existing
access JWTs expire independently. A lost response is not evidence of rollback. Missing generation
fields fail closed, requiring an explicit data decision for an existing populated deployment.

Current limits include separate verification-proof/account writes, multi-step lifecycle cleanup,
composite rather than independent [rate limits](store/rate-limits.md), and selected
[audit records](observability/audit.md) persisted without TTL.

The [test overview](testing.md) distinguishes memory-backed coverage from deployed or local-DynamoDB
evidence. The [optional examples](../../examples/README.md) have their own session and platform limits.
