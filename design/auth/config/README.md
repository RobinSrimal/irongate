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

- `account-lifecycle.md`: deleted identity reuse and retention policy.
- `clients.md`: config-only OAuth clients.
- `client-file.md`: checked-in TOML client definitions with secret refs.
- `environment.md`: runtime environment parsing and validation.
- `stages.md`: stage-sensitive auth behavior.
- `ttls.md`: token and short-lived auth artifact TTL configuration.
