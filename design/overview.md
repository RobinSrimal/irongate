# Overview

This document records the shape of the Irongate template.

Scope decisions live here because they cut across function code, infrastructure, and examples. The
design tree under `design/functions` and `design/infra` mirrors the code tree the template owns.

## What It Is

Irongate is a Rust and AWS auth template:

- Public auth Lambda for OAuth/OIDC, password auth, Google login, Apple login, refresh, revoke, and
  userinfo.
- IAM-protected admin Lambda for account lifecycle operations.
- DynamoDB-backed auth storage.
- SST-managed API Gateway, Lambda, DynamoDB, secrets, logs, and optional KMS resources.
- Optional examples for web BFF and native app integrations, with current limits documented in
  [example design](examples/README.md).

## Why

The template is intentionally small enough for developers to understand before they deploy it.
Security-sensitive behavior stays explicit:

- OAuth clients are config-only and reviewed through repo/deploy changes.
- Public auth routes never need a first-deployer bootstrap credential.
- Account lifecycle operations are isolated behind API Gateway IAM.
- DynamoDB stores typed auth records with HMAC lookup digests for bearer-style secrets.
- Email verification and password reset use Resend in every deployed stage.
- The auth core stays API-only so applications own their user-facing product experience.

## Core Auth Function

The public auth function owns:

- OAuth authorize, token, discovery, JWKS, revoke, and userinfo endpoints.
- OpenID Connect-compatible ID-token issuance.
- Config-only OAuth client loading and validation.
- Password registration, login, email verification, and password reset.
- Argon2id password hashing with length-based password policy.
- Google and Apple OIDC login.
- Persisted minimal account and identity records.
- Proof-bound email ownership for password completion and new Google/Apple identities.
- Refresh-token rotation and user-facing refresh-token revocation.
- Rate limiting and audit logging.

The auth function returns protocol responses, JSON, redirects, and OAuth errors. Application login,
registration, reset, provider-selection, and error screens live in the consuming app or optional
examples.

### Password Registration

The [password API](functions/auth/api/providers/password.md) accepts only email at registration.
Pending state has a fresh `registration_id`, no hash, subject or email ownership; resends reuse
that instance.
Verification requires the emailed proof plus a new password. The domain validates policy, prepares
the proof/binding/identity and email-owner eligibility checks without consumption, then hashes. One
[eight-operation transaction](functions/auth/src/store/registration.md) consumes the proof and writes
the verified credential, fresh generation-zero account, password identity, email owner and three
subject indexes. An active owner blocks completion rather than attaching a password to its subject,
even when that account is disabled, deleted or missing.
Replay and sibling links cannot replace credentials. Completion is not login and returns no tokens.

The existing optional web example renders its verification GET/HEAD without auth calls or mutations;
only a deliberate same-origin POST completes setup. There are no core-hosted pages or new services,
hosting requirements or infrastructure changes. The auth deployment remains independent of examples.

The contract requires a bound pending registration with no preinstalled hash; unbound proofs are
invalid. Login/recovery uses the persisted credential and account, not email-based account selection.
There are no compatibility paths, migrations or backfills. See
[email setup](../docs/setup/06-resend-email.md) for the operator boundary.

[Email owners](functions/auth/src/store/email_ownership.md) use `email:owner` keyed by the existing
normalized-email HMAC digest, plus `email_by_subject:{subject}` for indexed cleanup. Values contain
no raw email or credentials and have no TTL. Deletion clears the owner subject and removes its
index together after existing credential/session cleanup; retention governs reuse, not expiry.
Disable, enable and reset do not transfer ownership. Google and Apple use this ownership boundary
for new identities with provider-bound authority; password completion rejects active owners.

### Provider Sign-In

Google browser callbacks require an independent per-transaction issuer-host cookie before state or
session consumption. Only its HMAC is persisted. Typed state consumption checks provider, proof,
expiry and the exact record; invalid proof leaves the transaction usable. State/cookie lifetime is
capped by the authorize session, and confirmed consumption clears only that cookie. This supplements
OAuth state/PKCE/nonce, not application sessions. Apple retains its existing form-post flow.

Google/Apple callbacks validate provider tokens, then use typed prepare/finish
[provider issuance](functions/auth/store/identities.md). Preparation pins the identity and active
account generation without writes. Returning issuance checks the exact account while replacing
the exact identity and creating the code. New Google/Apple issuance requires provider-bound email
authority: six operations guard or create the email owner/account/index, write the identity and its
subject index, and create the code.
Selected-provider match and session expiry are checked at preparation; expiry is checked
again immediately before dispatch. There is one application-level transaction attempt, without
re-resolving a different subject/generation on failure.

Provider state and authorize sessions are consumed earlier, before exchange/validation. Failure
requires restarting sign-in. Confirmed issuance rejection returns a fixed `400 invalid_grant`;
operational errors, transaction conflicts and uncertain outcomes return a fixed `500 server_error`,
not a claim of invalid credentials or rollback. A lost response can follow a committed issuance.

Issuer/subject remains the provider identity key, taking precedence over later email changes.
New Google/Apple identities may share an account through validated ownership, never through
unsigned profile data or pending credentials. Relay addresses remain distinct from their hidden
destinations. Missing/inactive owner accounts or malformed ownership indexes fail closed.
Google requires verified Gmail or valid signed hosted-domain authority. A new Google identity with
third-party email and no such authority is rejected by the public callback. The
[confirmation store](functions/auth/src/store/sign_in_confirmation.md) can atomically consume
browser/email proof with pinned identity/code issuance. Its internal
[delivery operation](functions/auth/src/providers/email_confirmation.md) creates and sends a code
under independent source/email limits, preserving the actual stored deadline and uncertain-outcome
semantics. No public callback, confirmation route or UI calls it; verification-attempt admission
and confirmation browser/CSRF transport remain absent.
Existing accounts are never merged. Password-first then provider sign-in can share a subject;
adding a password to a provider-first account is not supported. This needs no additional routes,
configuration, dependencies or infrastructure.

## Admin Function

The admin function owns IAM-protected account lifecycle operations:

- Get sanitized account status.
- Disable a user.
- Enable a user.
- Delete a user.
- Revoke sessions for a user.

Operators call these routes with AWS Signature Version 4. API Gateway authorizes the request before
invoking the admin Lambda, and the admin Lambda returns sanitized account lifecycle responses rather
than raw DynamoDB records.

Delete returns success only after its indexed ownership cleanup finishes. A failure remains visible
and the existing delete action can be retried; earlier status/cleanup writes are not rolled back.
Each owner transition and exact index deletion is conditional, not a whole-account atomic deletion
or a new receipt workflow. Ownership records are not exposed in admin responses.

## Examples

Examples are optional reference implementations:

- `packages/examples/web`: Cloudflare Worker BFF using HttpOnly session cookies and Durable Object
  session storage.
- `packages/examples/app`: desktop-first Tauri app using PKCE, external browser login, loopback
  redirect, and OS keychain refresh-token storage.

Example infrastructure is opt-in through stage config. The default core deploy is only the auth/admin
Lambdas, API Gateway, DynamoDB, secrets, logs, and configured KMS resources.

## Tokens And APIs

Irongate issues self-contained JWT access tokens. Resource APIs validate issuer, audience, expiry,
signature, and scopes locally using discovery/JWKS metadata.

Password recovery atomically replaces the password, consumes its proof/index and increments an
internal account generation. Older reset proofs, codes and refresh credentials cannot authorize
new credentials. Code and refresh writes guard the matching active account; password login also
guards the exact verified password snapshot. DynamoDB reads are strongly consistent.

JWT signing and response delivery remain outside the storage transactions. An authorized in-flight
request may finish after recovery or lifecycle changes. Already-issued access JWTs remain valid at
independent resource servers until expiry, and userinfo retains its account-status check. Neither
reset nor disable/delete is an instantaneous barrier for every request or application session.
Exact account-value conditions detect state at commit, not disable-then-enable history that restores
the same value; provider issuance introduces no lifecycle epoch or new JWT revocation guarantee.

ID tokens are for OIDC clients. Resource APIs should use access tokens for authorization and
row-level access control.
