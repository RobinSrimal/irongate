# Scopes

Target code: `packages/functions/auth/src/core/scopes.rs`

## Owns

- Supported OAuth/OIDC scope names.
- Scope parsing and validation.
- Scope-to-claim mapping.
- Refresh-token issuance policy inputs.

## Target Scopes

V1 should support a small OIDC-compatible scope set:

```text
openid
profile
email
offline_access
```

`openid` turns the request into an OpenID Connect authentication request and makes ID-token issuance required on the authorization-code token response.

`profile` allows profile-style user claims when the provider/core has safe values.

`email` allows `email` and `email_verified` claims when those values are available and verified according to provider-specific rules.

`offline_access` requests a refresh token. Refresh-token issuance still requires the OAuth client to allow the refresh-token grant.

V1 does not include a hosted consent screen. Allowed scopes are constrained by deployment client configuration, and any user-facing consent UX belongs to the application using the template unless a later hosted-consent design is added.

## Security Invariants

- Unknown scopes are rejected with an OAuth `invalid_scope` error.
- `openid` is required before issuing an ID token.
- `email` claims are not identity keys for Google or Apple identities.
- `offline_access` does not override client configuration.
- Lack of hosted consent must not cause scopes outside the configured client allow-list to be granted.
- Scope strings are normalized before storage and token issuance.
