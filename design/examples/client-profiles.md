# Example Client Profiles

Implementation: [client file parser](../functions/auth/src/config/client_file.md) and
[client registry](../functions/auth/src/core/clients.md).

## Current Examples

| Client | Profile | Authentication | Redirect |
| --- | --- | --- | --- |
| `web` | `spa` | Public, PKCE, no secret | Exact localhost or configured HTTPS callback |
| `app` | `native_desktop` | Public, PKCE, no secret | Registered loopback host/path with dynamic port |

The web application is a BFF because it keeps tokens server-side. Its current OAuth registration
and token requests are nevertheless public. Changing only the TOML entry to confidential breaks
the example: it has no client-secret exchange/revoke support.

## Core Profile Support

The core also supports `native_mobile` and `web_confidential`.
Mobile profiles accept claimed HTTPS or reverse-domain private-use redirects; the current app
example does not implement mobile platform callbacks. Confidential profiles require a configured
secret reference and matching token authentication method.

Public profiles require PKCE. Browser `allowed_origins` are exact CORS origins, separate from
redirect URIs. Redirects are exact except the native-desktop loopback port rule; wildcard redirects
are rejected. CORS is not authentication and does not govern native/server HTTP clients.

Client definitions are deployment configuration, not records created by runtime client CRUD.
See [operator setup](../../docs/setup/05-auth-clients.md).
