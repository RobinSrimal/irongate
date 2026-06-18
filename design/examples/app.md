# App Example

Target code: `packages/examples/app`

## Owns

- Desktop-first native app example.
- Tauri shell and native auth integration.
- External browser OAuth flow.
- OS keychain refresh-token storage.
- Future calls to protected API routes once the web example grows beyond the initial password-auth flow.
- Mobile-specific guidance in the README/design.

## Pattern

The app example uses the native public-client OAuth pattern:

```text
Tauri app
  -> external system browser
  -> Irongate authorize/token/refresh/revoke
  -> protected API routes in a later example slice
```

The app is a public client:

```text
client_type = "native_desktop"
pkce_required = true
token_endpoint_auth_method = "none"
```

The app must not embed a client secret.

## Desktop Flow

1. App generates PKCE verifier/challenge and OAuth state.
2. App starts a local loopback listener.
3. App opens the external system browser to Irongate `/authorize`.
4. Irongate redirects to the registered loopback callback using the runtime port.
5. App validates state and exchanges the authorization code.
6. App stores the refresh token in the OS keychain or credential manager.
7. App keeps access tokens in memory only.
8. App calls protected API routes directly with the access token once those routes exist.

## Token Storage

The default native example uses OS-backed secure storage:

| Platform | Storage expectation |
| --- | --- |
| macOS | Keychain |
| Windows | Credential Manager |
| Linux | Secret Service or platform credential store |

The code should hide platform details behind a small secure-token-store abstraction so users can replace the storage backend without changing the OAuth flow.

## Mobile Notes

The first implementation is desktop-first. Mobile is documented instead of implemented as a separate example.

Mobile differences:

- Use claimed HTTPS app links where possible.
- Universal Links are preferred on iOS.
- Android App Links are preferred on Android.
- Private-use custom schemes are a fallback, not the preferred production path.
- Use iOS Keychain or Android Keystore-backed storage.
- Use the external/system browser, not an embedded WebView.
- Keep the same PKCE, refresh rotation, logout, and protected API validation model.

Mobile client config should use:

```text
client_type = "native_mobile"
pkce_required = true
token_endpoint_auth_method = "none"
```

## Security Invariants

- External/system browser only.
- No embedded WebView login.
- No client secret in app binaries.
- PKCE is required.
- Refresh tokens are stored only in OS-backed secure storage.
- Access tokens are memory-only.
- Logout revokes refresh-token state and deletes local secure storage.
- Protected API calls validate access-token expiry through normal API errors and refresh only when needed.

## Out Of Scope

- Separate first mobile implementation.
- Tauri Stronghold as the default storage path.
- Browser BFF session cookies.
- Direct access to raw Irongate DynamoDB records.
