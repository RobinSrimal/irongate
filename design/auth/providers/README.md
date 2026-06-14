# Providers

Target code: `packages/functions/auth/src/providers`

## Owns

- Identity proof for each supported provider.
- Provider-specific configuration.
- Provider-specific validation.

## Target Providers

- Password.
- Google.
- Apple.

## Must Not Own

- OAuth client token issuance.
- DynamoDB item construction.
- User analytics.

Provider modules should return a verified identity. The OAuth core maps that identity to an internal subject and issues tokens.
