# Password HTTP API

Source: [packages/functions/auth/src/api/providers/password.rs](../../../../../../packages/functions/auth/src/api/providers/password.rs).

## What

JSON registration/verification/forgot/reset and form-encoded login handlers.

## Why

HTTP responses preserve the distinction between email proof, password authentication and OAuth continuation.

## How

Handlers apply endpoint limits using trusted source context and HMAC email identifiers where available, then map domain results to JSON or a 303 code redirect. Registration, verification and reset do not issue tokens; errors and audit events are mapped at this boundary.

Reset maps invalid proof, expiry, binding, generation and confirmed commit rejection to the same
invalid-reset response. Weak passwords retain policy errors; storage failures remain server errors.
The successful response follows confirmed atomic recovery, without changing the HTTP contract.
