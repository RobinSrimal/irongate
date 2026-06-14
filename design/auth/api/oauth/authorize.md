# Authorize Endpoint

Target code: `packages/functions/auth/src/api/oauth/authorize.rs`

## Owns

- Parse and validate `/authorize` requests.
- Load OAuth client configuration.
- Create short-lived authorization session state.
- Redirect to a selected provider or provider selection UI.

## Required Inputs

- `response_type`
- `client_id`
- `redirect_uri`
- `state`
- optional `scope`
- optional `provider`
- PKCE challenge fields

## Security Invariants

- `response_type` must be `code`.
- `redirect_uri` must exactly match the registered client.
- Unsupported PKCE methods must fail.
- Session state must be random, short-lived, and opaque to the browser.
- No tokens are issued from this endpoint.

## Store Operations

- `get_client`
- `create_authorize_session`
