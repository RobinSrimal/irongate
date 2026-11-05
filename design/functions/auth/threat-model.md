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
- Anonymous password preinstallation, registration-proof replay and stale/sibling proof completion.
- Password brute force and credential stuffing.
- Password reset replay, partial recovery and stale credential writers.
- Authorization code replay.
- Refresh token replay and race conditions.
- Spoofed forwarded headers for rate-limit bypass.
- Raw table read exposure.
- Provider callback CSRF/state replay.
- Partial provider identity/code issuance and stale identity/account/generation writers.
- OIDC token validation mistakes.

## Security Principles

- Keep the public auth Lambda free of runtime control-plane bootstrap.
- Use typed store operations instead of generic persistence primitives.
- Store lookup digests for bearer secrets, not raw values.
- Keep JWT private keys out of ordinary AuthTable reads.
- Use Resend for real verification/reset delivery in every stage.
- Use API Gateway request context for source IP, not forwarded headers.
- Keep raw auth state behind runtime and audited break-glass access only.
- Accept only email at registration; validate email proof and pending-instance binding before hashing
  the initial password. Commit proof consumption, credential, account, identity and indexes together.
- Commit reset proof consumption, password replacement and generation invalidation together.
- Guard code/refresh writes with the active account generation; guard password login with the
  exact verified password snapshot as well.
- Validate provider signatures and claims before identity resolution. Require exact nonblank key
  IDs and reject explicit non-signing JWK use; known-key validation failures do not force refresh.
- Commit provider identity/account work and the authorization code together using the exact prepared
  identity and active-account generation, or absence/exact tombstone conditions for a fresh subject.
- Charge independent source and email budgets before protected password work; OAuth/provider starts
  retain an aggregate source ceiling across changing clients or sessions. Keep limiter keys private
  and fail closed on invalid policies or storage failures.

## Current Coverage Limits

These principles describe the intended protection of the listed assets, not proof that every abuse
case is closed. Strongly consistent reads and classified transaction failures in
[storage](store/dynamodb.md) do not alone make workflows atomic. [Recovery](core/passwords.md)
has a guarded transaction; codes/refreshes retain generation and account conditions. JWT signing and
delivery remain separate, so in-flight authorized requests can complete after recovery and existing
access JWTs expire independently. A lost response is not evidence of rollback. Missing generation
fields fail closed, requiring an explicit data decision for an existing populated deployment.

[Provider issuance](store/identities.md) prepares private snapshots after signed-token validation,
checks session/provider match and logical expiry, and rechecks expiry immediately before dispatch.
New/reusable identities use four writes for account, identity, reverse index and code; returning
identities use an exact active-account check, exact identity replacement and absent-code write.
Confirmed conditional rejection leaves no partial issuance writes. One application-level attempt
means competing legitimate callbacks may fail; there is no re-resolution to a newer generation.

Provider-state/session consumes and upstream exchange remain earlier and outside issuance. Failure
requires fresh sign-in. Operational failures, transaction conflicts and uncertain responses use a
fixed server error rather than invalid credentials; no compensating account deletion follows an
error. An unreturned code may have committed and expires normally. Checks before dispatch do not
guarantee that server processing finishes before the expiry instant. Exact account equality is not
a history marker for disable/enable sequences that restore the same value, and already-issued JWTs
remain subject to their existing expiry semantics.

Provider identity is still issuer/subject, not email or display name. Matching emails across sign-in
methods do not merge accounts, changed/missing email cannot move a returning identity, and unsigned
Apple profile input cannot authorize identity or code claims. Shared email ownership, provider-email
confirmation and account linking are not implemented by this transaction boundary.

Email-only [registration](src/store/registration.md) binds proofs to one pending `registration_id`.
Preparation checks purpose, expiry, eligibility and binding before hashing; finish rechecks expiry
and uses exact-value/absence conditions for all six operations. Policy rejection retains the proof,
and replay/sibling proofs cannot replace a completed credential. Deleted-identity recreation uses a
fresh binding and subject under existing reuse policy. Old pending records/proofs are invalidated
without migration/backfill; completed password users remain unaffected.

Proof issuance and email delivery are separate: a send failure can leave pending state and an unused
proof. Registration retains an explicit already-registered response and does not promise enumeration
resistance. Expiry is checked before transaction dispatch, not by an instantaneous network commit;
TTL only cleans up. An operational/uncertain completion failure does not prove rollback. Account
lifecycle cleanup still spans multiple operations.

The existing [web example](../../examples/web/src/routes.md) renders verification GET/HEAD without
auth calls or mutations; a same-origin POST with proof and confirmed password completes setup.
Registration pages use no-store, CSP, escaped proof fields and origin-only referrers, not token-bearing
paths/queries. These are app-owned pages, not hosted core pages or a new infrastructure dependency.
This narrow form boundary is not a general security certification of every example route.

[Audit events](observability/audit.md) use bounded typed JSON and safe failure diagnostics, but delivery
is best effort and not atomic with state changes. Partial writes or total output failure can lose
evidence; operator identifiers and HMAC references are correlating data. Historical DynamoDB audit
rows without TTL remain untouched and outside CloudWatch retention. Audit sanitization does not
establish that unrelated operational/request logs are secret-free.

Independent [rate limits](store/rate-limits.md) constrain rotating identities but are not edge DDoS
protection. Targeted email throttling and legitimate shared NAT/BFF-source contention remain possible.
The source and secondary charges are conservative sequential writes, not an atomic quota transaction.
Application parsing and provider-configuration rejection can precede admission.

Provider validation tests isolate signature/key/claim failures and callback side effects. They
do not certify live provider key rotation or the HTTP cache lifecycle. Google/Apple web-code-flow
validators do not enforce native presenter/azp policy or nbf; this is not full OIDC conformance.

The [test overview](testing.md) distinguishes memory-backed coverage from deployed or local-DynamoDB
evidence. The [optional examples](../../examples/README.md) have their own session and platform limits.
