# App Example Infrastructure

## What And Why

The desktop example has no dedicated cloud infrastructure. It calls the same Irongate issuer as any
native public OAuth client, so it does not depend on the Cloudflare web deployment.

## How

The operator configures `IRONGATE_ISSUER_URL` and optionally `IRONGATE_CLIENT_ID` in the
app's launch environment. The registered native-desktop client supplies the loopback callback
contract. Refresh-token storage lives in the app's Rust keychain code.

`examples.app.enabled` exists in the stage configuration shape, but the
[example infrastructure module](index.md) does not create app resources or app-specific outputs.
There is no `infra/examples/app.ts` implementation.

See [app design](../../examples/app.md) and
[setup instructions](../../../docs/setup/12-tauri-app-example.md).
