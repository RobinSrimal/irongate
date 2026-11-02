# Auth Clients

## Goal

Configure OAuth clients that are allowed to use Irongate.

## Inputs Needed

- Client ID.
- Client type.
- Redirect URIs.
- Allowed scopes.
- Allowed origins for browser clients.
- Confidential client secret reference, if applicable.

## Files To Edit

- `auth.clients.toml`

## Client Types

Use explicit profiles:

```text
spa
native_mobile
native_desktop
web_confidential
```

The checked-in web example uses `spa` with PKCE and `token_endpoint_auth_method = "none"`.
Its Worker keeps tokens server-side as a BFF but does not authenticate as a confidential OAuth
client. The Tauri app uses `native_desktop`.

For a separate confidential integration, supply the referenced SST secret explicitly in the public
auth Lambda environment and implement the configured client authentication method in the caller.
Changing only the web example's profile to `web_confidential` will break its token requests.

## Validation

```bash
npm run typecheck
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml --test startup_config
```

The local tests exercise configuration contracts; runtime startup validates the deployed client
file. Deploy only after completing the separate deployment prerequisites.

## Done When

- Every redirect URI is exact, except native desktop loopback dynamic ports.
- Browser clients have exact `allowed_origins`.
- Public clients require PKCE.
- Confidential clients reference secrets by name only.
