# Errors

Target code: `packages/functions/auth/src/core/errors.rs`

## Owns

- Domain error categories.
- Mapping security-sensitive failures to safe public messages.

## Target Behavior

Core errors should distinguish operational causes internally while allowing the API layer to return safe OAuth errors externally.

Examples:

```text
InvalidClient
InvalidGrant
ExpiredChallenge
ReplayDetected
RateLimitExceeded
StoreConflict
ProviderUnavailable
```

## Security Invariants

- Login errors should not reveal whether an email exists.
- Token errors should not reveal valid token family internals.
- Internal provider and AWS errors should not leak secrets.
