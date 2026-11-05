# Google Identity Provider

Implementation: [Google provider helpers](../src/providers/google.md).

## Owns

- Google OIDC configuration.
- Token exchange with Google.
- Google ID token validation.
- Google JWKS fetch, warm-Lambda caching, and stale-key refetch.
- Validated Google claims and issuer/subject lookup-digest construction for the callback.

## Issuance Boundary

The [callback](../api/providers/google.md) passes validated claims and its consumed session to typed
[provider issuance](../store/identities.md). Preparation retains the identity and complete active-account
generation; finish commits identity/account work and the code together. Existing subjects are stable
despite changed/missing email, and equal emails do not link accounts. There is no provider-specific
resolve-then-issue helper or automatic retry with a newer account generation.

State/session consumption precedes exchange/validation and is outside issuance's transaction. A
failed attempt requires restarting sign-in. This storage boundary changes neither Google validation
nor the existing bounded missing-key JWKS refresh behavior.

## Security Invariants

- Validate issuer `https://accounts.google.com`.
- Validate audience against configured Google client ID.
- Validate signature, expiry, issued-at tolerance, and nonce.
- Require a nonblank Google ID-token `kid` and match it exactly to the selected RSA key.
- Reject explicit JWK alg other than RS256 or use other than sig; absent optional alg/use is valid.
- Cache Google JWKS in warm Lambda memory for a bounded time.
- Refetch Google JWKS once when the token `kid` is not present in the current key set.
- Identity key is Google issuer plus `sub`.
- Email is an attribute, not the primary identity key.
- Disabled or deleted accounts cannot sign in.

The MissingKey callback category also includes absent/blank IDs and allows at most one forced
refresh. Signature, claim and explicit key-metadata failures do not trigger that retry. Failed
validation creates no account/identity/code, though provider state and authorize session have
already been consumed. These are current web-code-flow checks, not native presenter or nbf policy.
