# 06_refresh_rotation_and_logout

## Status

Preliminary reminder only. Flesh this slice out after `05_token_exchange_signing_and_userinfo` is implemented and reviewed.

## Goal

Complete long-lived session support on top of the runtime-signed token exchange path.

At the end of this slice, clients should be able to receive refresh tokens when offline access is allowed, rotate those refresh tokens safely, detect refresh-token reuse, and revoke the current refresh-token family for user-facing logout.

## Design Docs To Recheck

- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/core/tokens.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/store/accounts.md`
- `design/auth/store/keys.md`
- `design/auth/observability/audit.md`

## Preliminary Scope

Likely in scope:

- Refresh-token issuance during authorization-code exchange when `offline_access` policy is satisfied.
- Refresh token storage by HMAC lookup digest.
- Refresh token subject/client index records.
- Atomic `grant_type=refresh_token` rotation.
- Refresh-token reuse detection.
- Refresh-token family revocation.
- User-facing `POST /oauth/revoke` for logout.
- Discovery metadata update for `refresh_token`, `offline_access`, and `revocation_endpoint`.
- Sanitized audit events for refresh rotation, reuse detection, and logout revocation.

Likely out of scope:

- Google or Apple login.
- Password reset.
- IAM-protected account lifecycle admin routes.
- KMS ES256 implementation.
- Token introspection.
- Opaque access tokens.
- Generic OAuth/OIDC provider support.

## Acceptance Reminders

- Raw refresh tokens never appear in DynamoDB keys, logs, or errors.
- Refresh rotation is atomic.
- Reuse of a replaced or revoked refresh token is detected.
- User-facing revocation is idempotent and does not reveal whether a refresh token exists.
- A client cannot revoke another client's refresh token.
- Refresh-token revocation does not revoke already-issued access JWTs.
- Discovery advertises refresh and revocation only after the routes work.

## Next Step After Slice 05

Before implementing this slice, replace this reminder with a full detailed slice plan using the same structure as slices 01-05.
