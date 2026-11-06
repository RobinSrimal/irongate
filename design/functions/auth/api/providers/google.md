# Google Provider API

Implementation: [Google HTTP handlers](../../src/api/providers/google.md).

## Owns

- Start Google OIDC login.
- Handle Google callback.
- Call provider validation code.

## Flow

```text
create provider state, independent browser proof, nonce and upstream PKCE
redirect to Google with a per-transaction issuer-host cookie
receive code and state plus matching browser cookie
check browser HMAC/provider/expiry and conditionally consume exact provider state
consume authorize session
exchange code
validate ID token
prepare issuer/sub identity and account snapshots; check session/provider and expiry
generate internal code; pass only its HMAC digest to finishing
recheck session expiry and atomically write identity/account work plus code
redirect to the exact client callback with code and original state
```

`GET /google/callback` preserves the existing exchange, signature/issuer/audience/nonce checks and
bounded JWKS refresh. It uses [typed provider issuance](../../store/identities.md) only after token
validation. Preparation writes nothing and pins the returning identity and complete active account,
including credential generation. Finishing does not reread a newer generation or resolve a new subject.

The [cookie](../../crypto/cookies.md) is required for all Google browser callbacks, including
provider errors. The state-specific host cookie carries an independent random proof; only its HMAC
is stored. It is Secure, HttpOnly, Lax and session/provider-TTL bounded. Wrong or duplicate proofs
cannot consume state/session or contact Google. Only confirmed state consumption clears that cookie
on success or handled terminal failure. Start/callback responses use no-store and no-referrer.
Browser navigation must visit the auth issuer; fetching start server-side and transplanting its
cookie to an app host is not supported. No hosted pages or application-session cookie are introduced.

A new or reusable deleted identity requires provider/identity-bound email authority: verified
Gmail or a valid signed hosted-domain claim. It commits six guarded operations for account, owner,
email index, identity, identity index and code. An existing owner requires its exact active account
and matching index; available email creates a fresh generation-zero account. Missing authority
rejects without a separate-account fallback. Third-party-email confirmation is not implemented.
A returning identity commits three operations: exact active-account check,
exact identity replacement and absent-code Put. The identity retains the current allowlist of
provider, issuer, email, email verification, name and picture; code properties contain only provider,
email and email verification. The signed `hd` claim is used only for authority classification,
not metadata or token claims. Raw properties and request parameters cannot authorize selection.

The consumed session supplies client/redirect, scope, downstream PKCE and OIDC nonce. Its selected
provider must match and its expiry must be valid during preparation; expiry is checked again
immediately before dispatch. The raw code reaches the client only after confirmed success. Callback
responses do not issue access, refresh or ID tokens; token exchange remains separate.

## Failure Boundary

There is one application-level issuance transaction attempt. Confirmed conditional rejection or
blocked identity reuse returns `400 invalid_grant` with `provider sign-in is no longer valid`.
Storage outages, transaction conflicts and uncertain outcomes return `500 server_error` with
`provider sign-in unavailable`, without raw record/provider/SDK values. Callback state/session
storage errors use the same mapper; missing or replayed state/session retains its earlier
`invalid_request` response. Upstream exchange and JWT/JWKS validation errors are separate.

After browser admission, provider state and session are consumed before exchange/validation; later failed callbacks require a
fresh sign-in, not replay or automatic re-resolution/upstream retry. Confirmed cancellation leaves
no partial issuance writes, but a lost response can follow a complete commit. No compensating
account deletion is performed; unreturned codes expire normally. These guards do not track
disable/enable history that restores the same value or revoke already-issued JWTs instantaneously.

## Security Invariants

- Identity key is `issuer + sub`, not email.
- Require the starting browser's independent transaction proof before state/session consumption.
- Validate ID token signature, issuer, audience, expiry, and nonce.
- New identities may share an active owner only through provider-bound email authority; changed
  or missing claims on an existing identity retain the stored subject and never merge accounts.
- Store only minimal provider claims.
- Rate-limit provider-start by provider, authorize-session lookup digest, and trusted API Gateway source identity.
- Provider-start rate-limit keys must not contain raw authorize session keys.
- Returning issuance requires the prepared active account and identity to match at commit;
  allowed deleted-identity reuse may join an active owner or create a fresh account, never revive
  the old subject.
