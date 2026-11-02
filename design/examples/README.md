# Example Applications

The optional examples demonstrate two integration boundaries without selecting hosting or a
frontend framework for the auth core:

- [Web](web.md): TypeScript Cloudflare Worker BFF with server-held tokens and Durable Objects.
- [App](app.md): React/TypeScript Tauri desktop app with Rust auth commands and keychain refresh storage.

Both use Irongate's public protocol endpoints, not direct DynamoDB access. Password login and
configured Google/Apple browser login use authorization codes and PKCE. Examples can be adapted
or removed independently of the core.

## Configuration

The checked-in [client registry](../../auth.clients.toml) registers the web example as a public
`spa` client and the app as `native_desktop`. A server-side BFF token store does not itself
make the OAuth client confidential. See [client profiles](client-profiles.md).

Cloudflare deployment is disabled in both checked-in stages. The app runs locally and has no
dedicated cloud resources. See [example infrastructure](../infra/examples/README.md).

## Coverage

File-level mirrors live under `web/` and `app/`, preserving source/test/configuration paths.
The topic pages describe current security boundaries and limitations, not production certification.
In particular, the web example does not implement refresh handling or server-enforced session
expiry, and the native example returns access/ID tokens to frontend memory while keeping refresh
tokens in Rust-managed storage.
