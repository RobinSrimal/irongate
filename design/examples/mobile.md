# Mobile Example

Target code: `packages/examples/mobile`

## Owns

- Native mobile OAuth client flow.
- External/system browser launch.
- PKCE verifier/challenge generation.
- App link or custom-scheme redirect handling.
- OS secure storage for refresh tokens.
- Calling a protected resource API with access tokens.

## Client Profile

The mobile app is a public client:

```text
client_type = "native_mobile"
token_endpoint_auth_method = "none"
pkce_required = true
```

It must not use or embed a shared client secret. Secrets packaged into a mobile app are extractable.

## Browser Rule

Mobile auth must use the external/system browser or platform equivalent, such as iOS `ASWebAuthenticationSession` or Android Custom Tabs.

The example must not use an embedded WebView for login.

## Redirects

Preferred:

```text
https://app.example.com/mobile/callback
```

with platform-claimed HTTPS links, such as Universal Links or Android App Links.

Acceptable for examples:

```text
com.example.app:/oauth/callback
```

with a private-use custom scheme.

Redirects are still registered and validated. Wildcards are not allowed.

## Token Storage

- Store refresh tokens in OS secure storage, such as Keychain or Keystore.
- Keep access tokens short-lived.
- Rotate refresh tokens on every use.
- Revoke refresh-token state on logout.

## Security Invariants

- Authorization Code with PKCE only.
- No client secret.
- No embedded WebView.
- Validate `state`.
- Use `nonce` for OIDC login.
- Do not log tokens, codes, or redirect URLs containing codes.
- Treat local device compromise as outside what OAuth alone can solve.
