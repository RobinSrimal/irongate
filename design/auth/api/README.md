# Auth API

Target code: `packages/functions/auth/src/api`

## Owns

- HTTP route registration.
- Request extraction and response formatting.
- Mapping domain errors to HTTP/OAuth errors.
- Calling core/provider/store modules.

## Must Not Own

- DynamoDB expression details.
- Password hashing or token signing internals.
- Provider-specific ID token validation.
- Business application behavior.

## Target Modules

```text
api/
  oauth/
  providers/
```

The API layer should stay thin. Protocol decisions belong in `core`, provider identity proof belongs in `providers`, and persistence belongs in `store`.
