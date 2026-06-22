# Provider API

Target code: `packages/functions/auth/src/api/providers`

## Owns

- HTTP handlers for provider-specific login starts and callbacks.
- Translating provider proof into an internal OAuth authorization code.

## Providers

- Password.
- Google.
- Apple.

## Security Invariants

- Provider state must be random, short-lived, and single-use.
- Provider callbacks must not issue tokens directly to clients.
- Successful provider identity proof results in an internal subject, then an OAuth authorization code.
