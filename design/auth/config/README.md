# Auth Config

Target code: `packages/functions/auth/src/config`

## Owns

- Parsing auth runtime configuration.
- Validating provider configuration.
- Stage-sensitive defaults.

## Must Not Own

- SST resource creation.
- Provider HTTP calls.
- DynamoDB item operations.

Configuration should fail early and clearly when a deployed stage is missing required auth settings.

## Design Files

- `clients.md`: deployment-defined OAuth clients.
- `environment.md`: runtime environment parsing and validation.
- `stages.md`: stage-sensitive auth behavior.
