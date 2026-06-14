# Auth Core

Target code: `packages/functions/auth/src/core`

## Owns

- Protocol-independent auth domain types.
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
- `identities.md`: provider identity and linking rules.
- `passwords.md`: password auth domain rules.
- `subjects.md`: internal subject identifiers.
- `tokens.md`: token lifecycle rules.
- `errors.md`: domain error categories.
