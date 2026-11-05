# Apple Provider API

Implementation: [Apple HTTP handlers](../../src/api/providers/apple.md).

## Owns

- Start Sign in with Apple.
- Handle Apple callback.
- Call Apple-specific OIDC validation and client-secret generation.

## Implemented Boundaries

The API-only start route is:

```text
GET /apple/authorize?session=<raw-authorize-session-key>
```

The route validates the typed authorize session, stores HMAC-keyed provider state, and redirects to Apple with `response_mode=form_post`, `state`, `nonce`, and PKCE.

The API-only callback route is:

```text
POST /apple/callback
```

The callback consumes provider state and authorize session before exchange/validation, then uses
[typed provider issuance](../../store/identities.md) to commit identity/account work and the internal
OAuth authorization code together. The callback method and downstream token-exchange contract are unchanged.

## Flow

```text
create provider state and nonce
redirect to Apple
receive code and state
consume provider state
consume authorize session
generate Apple client-secret JWT
exchange code
validate ID token
prepare issuer/sub identity and account snapshots; check session/provider and expiry
generate internal code; pass only its HMAC digest to finishing
recheck session expiry and atomically write identity/account work plus code
redirect to the exact client callback with code and original state
```

Preparation writes nothing and pins the returning identity and complete active account, including
credential generation. New/reusable identities use four conditional writes for a fresh generation-zero
account, identity, subject index and code; the identity checks absence or the exact tombstone, and
the other records require absence. Returning identities use an exact account check, exact identity
replacement and absent-code Put. Finishing cannot adopt a different subject or newer generation.

The consumed session supplies client/redirect, scope, downstream PKCE and OIDC nonce; preparation
requires matching selected provider and valid expiry, and finish rechecks expiry immediately before
dispatch. The raw code reaches the client only after confirmed success, with no access/refresh/ID
tokens in the callback response.

Identity properties retain provider, issuer, signed email/email verification and private-email flag.
Code properties contain only provider, email and email verification. Changed or absent signed email
cannot move an existing identity; unsigned callback profile data does not authorize identity/code
claims. Private-relay addresses do not imply ownership of another address or authorize account linking.

## Failure Boundary

There is one application-level issuance transaction attempt. Confirmed conditional rejection or
blocked identity reuse returns `400 invalid_grant` with `provider sign-in is no longer valid`.
Storage outages, transaction conflicts and uncertain outcomes return `500 server_error` with
`provider sign-in unavailable`, without raw record/provider/SDK values. Callback state/session
storage errors use the same mapper; missing or replayed state/session retains its earlier
`invalid_request` response. Upstream exchange and JWT/JWKS validation errors are separate.

Earlier state/session consumption is not rolled back by later failure: restart sign-in rather than
replay the callback. No automatic re-resolution or upstream exchange retry is added. Confirmed
cancellation has no partial issuance writes; a lost response may follow a complete commit, so no
compensating account deletion occurs. Unreturned codes expire normally. Exact-value guards do not
track restored disable/enable history or provide instantaneous JWT revocation.

## Security Invariants

- Treat Apple as first-class OIDC, not generic OAuth2.
- Validate ID token signature, issuer, audience, expiry, and nonce.
- Apple private key material must come from secrets, not DynamoDB.
- Do not rely on Apple email as the canonical identity.
- Rate-limit provider-start by provider, authorize-session lookup digest, and trusted API Gateway source identity.
- Provider-start rate-limit keys must not contain raw authorize session keys.
- Returning issuance requires the prepared active account and identity to match at commit;
  allowed deleted-identity reuse creates a fresh subject, never reviving a deleted account.
