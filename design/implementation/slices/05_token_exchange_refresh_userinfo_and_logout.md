# 05_token_exchange_refresh_userinfo_and_logout

## Status

Preliminary reminder only. Flesh this slice out after `04_password_login_and_authorization_code` is implemented and reviewed.

## Goal

Complete the first target OAuth/OIDC loop after slice 04 can issue typed authorization codes.

At the end of this slice, a client should be able to exchange a typed authorization code for tokens, refresh those tokens safely, call `/userinfo`, and revoke refresh-token state for user-facing logout.

## Design Docs To Recheck

- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/core/tokens.md`
- `design/auth/crypto/signing.md`
- `design/auth/store/authorization-codes.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/store/accounts.md`
- `design/auth/store/identities.md`
- `design/auth/store/keys.md`
- `design/auth/observability/audit.md`

## Reminders From Slice 01-03 Review

### Runtime Signing And JWKS Must Be Cut Over First

Runtime config already loads the configured signer. Target token paths and JWKS must use that runtime signer instead of the legacy DynamoDB `signing:key` storage path.

Required direction:

- `/.well-known/jwks.json` returns public keys from the configured runtime signer.
- Access-token and ID-token signing use the configured runtime signer.
- Token verification paths use the same configured public key material.
- Target code does not create, scan, or read plaintext JWT private keys from `AuthTable`.
- `kms-es256` can remain unimplemented in this slice if startup fails clearly when selected.

### Discovery Must Match Mounted Routes

Do not advertise `/oauth/revoke` until the route is mounted and backed by typed refresh-token revocation.

Before this slice is complete:

- Discovery either omits `revocation_endpoint`, or
- Discovery advertises it only after `POST /oauth/revoke` is implemented and tested.

## Preliminary Scope

Likely in scope:

- Consume typed authorization-code records created by slice 04.
- Validate PKCE during authorization-code exchange.
- Issue ES256 JWT access tokens through the configured runtime signer.
- Issue OIDC ID tokens when `openid` was granted.
- Store refresh tokens by HMAC lookup digest, never raw refresh tokens.
- Rotate refresh tokens atomically.
- Detect refresh-token reuse and revoke related refresh-token state.
- Add user-facing `POST /oauth/revoke` for logout.
- Cut `/userinfo` over to target account/identity data.
- Update discovery metadata only for routes and algorithms that are actually supported.

Likely out of scope:

- Google or Apple login.
- Password reset.
- IAM-protected account lifecycle admin routes.
- KMS ES256 implementation.
- Token introspection.
- Opaque access tokens.
- Generic OAuth/OIDC provider support.

## Acceptance Reminders

- No runtime token path reads JWT private keys from DynamoDB.
- JWKS and token signing use the same configured key source.
- Authorization codes are single-use and consumed through typed store operations.
- PKCE is enforced at token exchange.
- ID tokens are issued only for OIDC requests.
- Refresh tokens are stored and looked up by HMAC digest.
- Refresh rotation is atomic.
- Refresh-token reuse is detected and audited.
- `/oauth/revoke` exists before discovery advertises it.
- `/userinfo` exposes only intended user claims.
- No access token, refresh token, ID token, authorization code, or refresh-token digest is logged or stored in DynamoDB keys.

## Next Step After Slice 04

Before implementing this slice, replace this reminder with a full detailed slice plan using the same structure as slices 01-04:

- concrete store records
- route behavior
- token claim shapes
- error behavior
- focused tests
- manual validation notes
