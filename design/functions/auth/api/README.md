# Auth API

Target code: `packages/functions/auth/src/api`

## Owns

- HTTP route registration.
- Request extraction and response formatting.
- Mapping domain errors to HTTP/OAuth errors.
- Calling core/provider/store modules.

## Boundaries

- DynamoDB expression details.
- Password hashing or token signing internals.
- Provider-specific ID token validation.
- Business application behavior.
- HTML page rendering or product login UI.

## Target Modules

```text
api/
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

Operator-only account lifecycle routes live under `design/functions/admin`. The public auth API must
not mount `/_admin/*` routes behind the `$default` Lambda integration.
