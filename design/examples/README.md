# Example Applications

Target code:

```text
packages/examples/
  auth-web/
  web-spa/
  mobile/
  desktop/
  resource-api/

infra/examples/
```

## Purpose

Examples should demonstrate secure ways to use Irongate from web, mobile, and desktop applications without making any frontend or hosting choice part of Irongate core.

Irongate core remains:

```text
API Gateway
  -> public Rust auth Lambda
  -> IAM-protected Rust admin Lambda
  -> DynamoDB
  -> optional KMS
```

Examples are optional reference implementations. They may be copied, adapted, or ignored by template users.

## Example Set

| Example | Purpose |
| --- | --- |
| `auth-web` | Optional browser-hosted login, registration, verification, reset, and provider-selection surface. |
| `web-spa` | Static browser app using Authorization Code + PKCE against Irongate. |
| `mobile` | Native mobile client using the external system browser, PKCE, app/claimed redirects, and OS secure storage. |
| `desktop` | Native desktop client using the external system browser, PKCE, loopback redirect, and OS keychain storage. |
| `resource-api` | Minimal protected API that validates Irongate access JWTs. |

## Security Posture

Examples should demonstrate high-security defaults:

- Authorization Code flow with PKCE.
- No implicit flow.
- No resource-owner password grant.
- No client secrets in browser or native app code.
- Exact redirect matching, except native desktop loopback dynamic ports.
- Strict browser CORS origins.
- Short access-token lifetime.
- Refresh-token rotation and reuse detection when refresh tokens are used.
- OS secure storage for native refresh tokens.
- No third-party scripts or analytics on auth pages by default.

## Deployment Boundary

Examples deploy only when explicitly enabled:

```text
examples.enabled = false
```

The default deploy must not import or create example frontend hosting, sample resource APIs, Cloudflare resources, S3 buckets, CloudFront distributions, or native build tooling.

## Design Files

- `auth-web.md`: optional hosted login surface.
- `web-spa.md`: browser SPA integration.
- `mobile.md`: native mobile integration.
- `desktop.md`: native desktop integration.
- `resource-api.md`: protected API validation.
- `client-profiles.md`: OAuth client profile rules for examples.
