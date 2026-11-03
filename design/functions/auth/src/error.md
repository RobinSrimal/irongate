# Error Responses

Source: [packages/functions/auth/src/error.rs](../../../../packages/functions/auth/src/error.rs).

## What

OAuth, auth, storage and umbrella error types.

## Why

Domain failures need protocol status codes without exposing server internals.

## How

IntoResponse maps OAuth failures to JSON using caller-provided descriptions, including ServerError.
Auth errors use endpoint-specific statuses/text and Retry-After for limits. The umbrella storage
and internal error responses use generic 500 text but log the wrapped error. StorageError separates
not-found, conditions, conflicts and backend failures; individual callers choose the public mapping.
Transaction errors are sanitized in the adapter, not by a blanket redaction rule here.
