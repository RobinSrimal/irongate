# Error Responses

Source: [packages/functions/auth/src/error.rs](../../../../packages/functions/auth/src/error.rs).

## What

OAuth, auth, storage and umbrella error types.

## Why

Domain failures need protocol status codes without exposing server internals.

## How

IntoResponse maps OAuth and auth failures to JSON, adds Retry-After for limits and uses generic descriptions for server errors. StorageError separates not-found, conditions, conflicts and backend failures; individual callers choose the public mapping.
