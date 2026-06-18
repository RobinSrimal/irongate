# Example Applications

Target code:

```text
packages/examples/
  web/
  app/

infra/examples/
```

## Purpose

Examples should demonstrate best-practice ways to use Irongate from web and native applications without making any frontend or hosting choice part of Irongate core.

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
| `web` | Cloudflare Worker web app using a BFF. Browser receives only an HttpOnly Secure SameSite session cookie; refresh tokens stay server-side in Durable Objects. The first slice covers password auth only. |
| `app` | Desktop-first Tauri native app using the external system browser, PKCE, loopback redirect, and OS keychain storage. Mobile-specific differences are documented. |

## Security Posture

Examples should demonstrate high-security defaults:

- Authorization Code flow with PKCE.
- No implicit flow.
- No resource-owner password grant.
- No client secrets in browser or native app code.
- No browser refresh-token storage.
- Exact redirect matching, except native desktop loopback dynamic ports.
- Strict browser CORS origins.
- Short access-token lifetime.
- Refresh-token rotation and reuse detection when refresh tokens are used.
- OS secure storage for native refresh tokens.
- Server-side refresh-token storage for web sessions.
- Protected API routes validate Irongate access tokens before returning app data once those routes are added.
- No third-party scripts or analytics on auth pages by default.

## Deployment Boundary

Examples deploy only when explicitly enabled:

```text
examples.enabled = false
```

The default deploy must not import or create example frontend hosting, Cloudflare resources, S3 buckets, CloudFront distributions, or native build tooling.

## Design Files

- `web.md`: browser BFF integration.
- `app.md`: desktop-first Tauri native integration with mobile notes.
- `client-profiles.md`: OAuth client profile rules for examples.
