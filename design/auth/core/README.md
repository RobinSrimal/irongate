# Auth Core

Target code: `packages/functions/auth/src/core`

## Owns

- Protocol-independent auth domain types.
- Account lifecycle rules.
- OAuth client rules.
- Subject identity rules.
- Token lifecycle rules.
- Domain error types.

## Must Not Own

- HTTP framework details.
- DynamoDB SDK calls.
- External provider HTTP calls.
- Email delivery details.

The core should be testable without AWS.

## Design Files

- `clients.md`: OAuth client rules.
- `account-lifecycle.md`: account status, disable, delete, and session revocation rules.
- `identities.md`: provider identity and linking rules.
- `passwords.md`: password auth domain rules.
- `scopes.md`: OAuth/OIDC scope parsing and claim mapping.
- `subjects.md`: internal subject identifiers.
- `tokens.md`: token lifecycle rules.
- `errors.md`: domain error categories.
