# Auth Web Example

Target code: `packages/examples/auth-web`

## Owns

- Login form.
- Registration form.
- Email verification token page.
- Password reset token page.
- Provider selection for password, Google, and Apple.
- OAuth continuation UI for browser, mobile, and desktop clients.

## Must Not Own

- Token signing.
- Password verification.
- Account state.
- Provider callback validation.
- Authorization-code issuance.
- Token exchange.
- Refresh rotation.
- DynamoDB records.
- Resend API keys.

Those stay in Irongate core.

## Target Flow

```text
client application
  -> browser/system browser
  -> auth-web
  -> Irongate public API
  -> registered redirect URI with authorization code
  -> client exchanges code with PKCE
```

`auth-web` is an optional login surface in front of the API-only auth core. It lets web, mobile, and desktop examples use one consistent browser-based authentication UX without embedding that UI in the auth Lambda.

## Security Invariants

- No client secrets in browser code.
- No access token or refresh token storage by default.
- No analytics or third-party scripts by default.
- Strict Content Security Policy.
- No tokens, authorization codes, reset links, verification links, or provider credentials in logs.
- Preserve and verify OAuth `state`.
- Use OIDC `nonce` when requesting ID tokens.
- Do not use embedded WebViews for native applications.
- Do not call privileged admin routes.

## Hosting

Hosting is example infrastructure, not core infrastructure. Future deployments may use Cloudflare Pages, S3/CloudFront, or another static hosting target, but this repo should keep that choice opt-in under `infra/examples`.

## Relationship To Core

Irongate core remains API-only. If a developer wants custom UI, they can ignore this example and call the same public auth endpoints from their own application.
