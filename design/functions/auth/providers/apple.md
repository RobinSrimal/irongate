# Apple Identity Provider

Implementation: [Apple provider helpers](../src/providers/apple.md).

## Owns

- Sign in with Apple configuration.
- Apple client-secret JWT generation.
- Apple authorization URL construction.
- Token exchange with Apple.
- Apple ID token validation.
- Apple JWKS fetch, warm-Lambda caching, and stale-key refetch.
- Validated Apple claims and issuer/subject lookup-digest construction for the callback.

## Callback Flow

The API handler coordinates these provider helpers with typed storage:

```text
receive Apple code and state
consume provider state
consume authorize session
generate client-secret JWT
exchange code
validate ID token
prepare issuer/sub identity and exact active-account/generation snapshots
recheck consumed session expiry
atomically commit identity/account work and OAuth authorization code
```

The [callback](../api/providers/apple.md) uses [typed provider issuance](../store/identities.md) after
signed-token validation. Preparation checks provider selection and session expiry without writes;
finish commits four conditional writes for new/reusable identities or three operations for returning
ones. It cannot reread a newer generation or move an existing subject based on email. Unsigned
callback profile data is not identity/code authority; equal emails and private-relay claims do not
link accounts.

State/session consumption and upstream exchange remain outside the issuance transaction. Failure
requires restarting sign-in, not replay or automatic re-resolution. Apple validation and bounded
missing-key JWKS refresh are unchanged; operational/conflict/uncertain storage errors are not
converted into invalid credentials.

## Security Invariants

- Validate issuer `https://appleid.apple.com`.
- Validate audience against configured Apple client ID.
- Validate signature, expiry, issued-at tolerance, and nonce.
- Require a nonblank Apple ID-token `kid` and match it exactly to the selected RSA key.
- Reject explicit JWK alg other than RS256 or use other than sig; absent optional alg/use is valid.
- Cache Apple JWKS in warm Lambda memory for a bounded time.
- Refetch Apple JWKS once when the token `kid` is not present in the current key set.
- Apple private key comes from secrets.
- Apple client-secret JWTs are generated at runtime and are not stored.
- Identity key is Apple issuer plus `sub`.
- Do not assume email or name is always present.
- Disabled or deleted accounts cannot sign in.

The MissingKey callback category also includes absent/blank IDs and allows at most one forced
refresh. Signature, claim and explicit key-metadata failures do not trigger that retry. Failed
validation creates no account/identity/code, though provider state and authorize session have
already been consumed. Apple's provider ID-token verification remains separate from ES256
client-secret signing. This module does not enforce nbf or native presenter policy.
