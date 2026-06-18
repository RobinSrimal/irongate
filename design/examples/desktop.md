# Desktop Example

Target code: `packages/examples/desktop`

## Owns

- Native desktop OAuth client flow.
- External/system browser launch.
- PKCE verifier/challenge generation.
- Loopback redirect listener.
- OS keychain or credential-manager token storage.
- Calling a protected resource API with access tokens.

## Client Profile

The desktop app is a public client:

```text
client_type = "native_desktop"
token_endpoint_auth_method = "none"
pkce_required = true
```

It must not use or embed a shared client secret.

## Redirects

Desktop examples should use a loopback redirect:

```text
http://127.0.0.1:{dynamic_port}/oauth/callback
```

Target redirect validation:

- Scheme is `http`.
- Host is loopback only, such as `127.0.0.1`, `[::1]`, or `localhost` if explicitly allowed.
- Path must match the registered path exactly.
- Runtime port may vary.
- Dynamic port matching is allowed only for `native_desktop`.

This requires a future auth-core code slice. Until then, desktop examples can only use whatever redirect matching the core already supports.

## Token Storage

- Store refresh tokens in OS keychain or credential manager.
- Keep access tokens short-lived.
- Rotate refresh tokens on every use.
- Revoke refresh-token state on logout.

## Security Invariants

- Authorization Code with PKCE only.
- No client secret.
- External/system browser only.
- Validate `state`.
- Use `nonce` for OIDC login.
- Bind loopback listener to loopback only.
- Stop the loopback listener after receiving the callback.
- Do not log tokens, authorization codes, or callback URLs containing codes.
