# App Example Infra

Target code: `infra/examples/app.ts`

## Owns

- Native app support outputs when the app example exists.
- Optional app-facing configuration values such as issuer URL and client ID.

## Boundaries

- Cloudflare web Worker deployment and browser BFF sessions live in the web example.
- Irongate auth core AWS resources and the DynamoDB auth table live under `infra/auth`.
- Native token storage lives in app code through the OS keychain, not infrastructure.

## Deployment Boundary

The app example is disabled by default:

```text
examples.enabled = false
examples.app.enabled = false
```

The app example runs locally and is configured with:

```text
IRONGATE_ISSUER_URL
IRONGATE_CLIENT_ID
```

App infrastructure is imported only when `examples.app.enabled = true`.

## Relationship To Web

The app example is not a sub-feature of the web example. It should use Irongate directly as a native
OAuth public client:

```text
Tauri app
  -> external system browser
  -> Irongate authorize/token/refresh/revoke
  -> OS keychain refresh-token storage
```

Protected application APIs are an application choice, not a requirement of Irongate core or app
infrastructure.
