# Authorize Endpoint

Target code: `packages/functions/auth/src/api/oauth/authorize.rs`

## Owns

- Parse and validate `/authorize` requests.
- Load OAuth client configuration.
- Create short-lived authorization session state.
- Redirect to the selected provider when `provider` is supplied.
- Return a safe protocol error when provider choice is required but absent.

## Required Inputs

- `response_type`
- `client_id`
- `redirect_uri`
- `state`
- optional `scope`
- optional `nonce`
- optional `provider`
- PKCE challenge fields

## OIDC Parameter Boundary

V1 supports authorization-code flow plus OIDC `scope=openid` and optional client `nonce`.

Other OIDC optional authorization parameters, such as `prompt`, `max_age`, `claims`, `request`, `request_uri`, `display`, `ui_locales`, and `id_token_hint`, are not part of the first core unless separately designed. If a request uses an unsupported parameter that changes authentication semantics, the endpoint should fail safely instead of silently pretending to honor it.

Because the target core is API-only, `/authorize` does not display a hosted login, consent, account-selection, or provider-selection screen. Clients that expect a hosted OIDC login page need app-owned UI in front of these endpoints or a later hosted-UI design.

The optional `auth-web` example is such an app-owned UI. It is not part of the auth Lambda and does not change the API-only core boundary.

## Security Invariants

- `response_type` must be `code`.
- `redirect_uri` must exactly match the registered client.
- Native desktop loopback redirects may allow dynamic ports only for configured `native_desktop` clients.
- Unsupported PKCE methods must fail.
- OIDC requests with `openid` may include `nonce`; when supplied, it must be carried through to the authorization code and initial ID token.
- Session state must be random, short-lived, and opaque to the browser.
- No tokens are issued from this endpoint.
- The endpoint does not render provider-selection UI.

## Store Operations

- `create_authorize_session`

## Config Dependencies

- Read-only client registry lookup by `client_id`.
- `AUTH_AUTHORIZE_SESSION_TTL_SECONDS`.
- `AUTH_PROVIDER_STATE_TTL_SECONDS` when redirecting to Google or Apple.
