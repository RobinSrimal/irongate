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
- Refresh-token rotation and user-facing refresh-token revocation.
- Rate limiting and audit logging.

The auth function returns protocol responses, JSON, redirects, and OAuth errors. Application login,
registration, reset, provider-selection, and error screens live in the consuming app or optional
examples.

### Password Registration

The [password API](functions/auth/api/providers/password.md) accepts only email at registration.
Pending state has a fresh `registration_id`, no hash and no subject; resends reuse that instance.
Verification requires the emailed proof plus a new password. The domain validates policy, prepares
the proof/binding/identity checks without consumption, then hashes. One
[six-operation transaction](functions/auth/src/store/registration.md) consumes the proof and writes
the verified credential, fresh generation-zero account, password identity and two subject indexes.
Replay and sibling links cannot replace credentials. Completion is not login and returns no tokens.

The existing optional web example renders its verification GET/HEAD without auth calls or mutations;
only a deliberate same-origin POST completes setup. There are no core-hosted pages or new services,
hosting requirements or infrastructure changes. The auth deployment remains independent of examples.

This is one clean contract: old pending rows without a binding or with a preinstalled hash cannot
be reused, and old unbound proofs are invalid. A fresh link alone cannot repair an old pending row.
Use fresh development state or separately reviewed cleanup before deployment; there is no automatic
erasure, migration or backfill. Existing completed users and credentials remain unaffected. See
[email setup](../docs/setup/06-resend-email.md) for the operator boundary.

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

ID tokens are for OIDC clients. Resource APIs should use access tokens for authorization and
row-level access control.
