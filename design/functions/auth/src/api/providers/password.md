# Password HTTP API

Source: [packages/functions/auth/src/api/providers/password.rs](../../../../../../packages/functions/auth/src/api/providers/password.rs).

## What

JSON registration/verification/forgot/reset and form-encoded login handlers.

## Why

HTTP responses preserve the distinction between email proof, password authentication and OAuth continuation.

## How

Handlers enforce admission before password/proof lookup, hashing, email delivery or code issuance.
Registration, login and forgot use independent source and normalized-email budgets; verify/reset
use source only. Invalid emails consume source allowance without allocating arbitrary email rows.
The limiter receives trusted context and a typed identity, not a raw key. Domain results map to JSON
or a 303 code redirect. Registration, verification and reset do not issue tokens; errors and audit
events are mapped at this boundary.

Reset maps invalid proof, expiry, binding, generation and confirmed commit rejection to the same
invalid-reset response. Weak passwords retain policy errors; storage failures remain server errors.
The successful response follows confirmed atomic recovery, without changing the HTTP contract.
