# App Example Infra

Target code: `infra/examples/app.ts`

## Owns

- Native app support outputs when the app example exists.
- Optional app-facing configuration values such as issuer URL and client ID.
- Future app download or release helper outputs if explicitly designed later.

## Must Not Own

- Cloudflare web Worker deployment.
- Browser BFF sessions.
- Irongate auth core AWS resources.
- Irongate DynamoDB auth table.
- Native token storage. OS keychain storage lives in the app code, not infrastructure.
- Shared resource API infrastructure.

## Deployment Boundary

The app example is disabled by default:

```text
examples.enabled = false
examples.app.enabled = false
```

The first app example should not require deployed app infrastructure. A Tauri desktop app can be run
locally and configured with:

```text
IRONGATE_ISSUER_URL
IRONGATE_CLIENT_ID
```

Any future app infrastructure must be opt-in and must not be imported by the default auth-core deploy.

## Relationship To Web

The app example is not a sub-feature of the web example. It should use Irongate directly as a native
OAuth public client:

```text
Tauri app
  -> external system browser
  -> Irongate authorize/token/refresh/revoke
  -> OS keychain refresh-token storage
```

If the app later calls protected routes hosted by the web Worker, that must be documented as an
application API choice, not a requirement of Irongate core or app infrastructure.
