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
- HTML page rendering or product login UI.

## Target Modules

```text
api/
  admin.md
  oauth/
  providers/
```

The API layer should stay thin. Protocol decisions belong in `core`, provider identity proof belongs in `providers`, and persistence belongs in `store`.

## API-Only Boundary

The target auth service exposes protocol and provider endpoints only. It can return JSON responses, OAuth redirects, OAuth errors, and empty success responses, but it should not render forms or provider-selection pages.

Client applications own:

- Login screens.
- Registration screens.
- Password reset screens.
- Consent screens.
- Account-selection screens.
- Provider selection UI.
- Error presentation.

Email body templates are separate from auth UI. They live under `email/templates` because verification and reset emails are part of the auth protocol.

The API crate must not depend on a frontend framework or bundled application.

## Admin Boundary

The API layer also owns narrow operator-only account lifecycle routes under `/_admin/*`. These routes are not public auth UX and are not backed by custom admin credentials. They must be configured with API Gateway IAM authorization and call core lifecycle operations only.
