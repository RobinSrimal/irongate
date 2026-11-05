# Password HTTP API

Source: [packages/functions/auth/src/api/providers/password.rs](../../../../../../packages/functions/auth/src/api/providers/password.rs).

## What

JSON registration/verification/forgot/reset and form-encoded login handlers.

## Why

HTTP responses preserve the distinction between email proof, password authentication and OAuth continuation.

## How

`POST /password/register` accepts only `{email}` and returns `{status: "verification_required"}`.
`POST /password/verify` requires `{token, new_password}` and returns `{status: "verified", subject}`
after atomic completion. Both request DTOs reject unknown fields: email/password registration and
token-only verification are not supported. Neither endpoint returns an OAuth code or auth tokens,
sets a session, or exposes the domain's delivery ID.

Handlers enforce admission before password/proof lookup, hashing, email delivery or code issuance.
Registration, login and forgot use independent source and normalized-email budgets; verify/reset
use source only. Invalid emails consume source allowance without allocating arbitrary email rows.
The limiter receives trusted context and a typed identity, not a raw key. Domain results map to JSON
or a 303 code redirect. Registration, verification and reset do not issue tokens; errors and audit
events are mapped at this boundary.

Registration preserves the explicit already-registered error; it does not promise enumeration
resistance. Old pending rows without an ID or with a hash are also ineligible for registration reuse;
requesting another link alone does not repair them. Verification maps invalid, expired,
wrong-purpose, mismatched and replayed proofs or confirmed finish rejection to one `invalid_grant`.
Password-policy failure is `invalid_request` and does not consume the proof.
Registration/verification storage errors use fixed safe `server_error`
messages; delivery failure is `temporarily_unavailable`. These errors do not echo submitted values
or raw storage errors. Old proofs with absent, empty or non-string registration IDs are rejected as
invalid proof before typed decoding, not as storage failures. See the
[API contract](../../../api/providers/password.md).

Reset maps invalid proof, expiry, binding, generation and confirmed commit rejection to the same
invalid-reset response. Weak passwords retain policy errors; storage failures remain server errors.
The successful response follows confirmed atomic recovery, without changing the HTTP contract.
