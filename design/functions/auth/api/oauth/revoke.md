# Revoke Endpoint

Target code: `packages/functions/auth/src/api/oauth/revoke.rs`

## Owns

- User-facing logout support.
- Refresh token revocation request parsing.
- OAuth client authentication/validation for revocation.
- Idempotent revocation responses.

## Endpoint

```text
POST /oauth/revoke
```

This endpoint supports ordinary app logout. The application still clears local browser/device state, such as local token storage or app cookies. The auth server revokes refresh-token state so the application cannot silently obtain new access tokens after logout.

## Request

The request accepts a refresh token:

```text
token=<refresh_token>
token_type_hint=refresh_token optional
```

Confidential clients must authenticate according to their configured token endpoint auth method. Public clients must identify the client and can only revoke refresh tokens that belong to that client.

Browser clients may call this endpoint only from configured `allowed_origins`. The auth router returns exact CORS origins and does not return wildcard origins.

## Target Behavior

- Look up the refresh token by HMAC digest.
- Verify the token belongs to the requesting client.
- Revoke the current refresh token family/session.
- Return success for already revoked, missing, or invalid tokens when client authentication is valid.
- Do not revoke other devices or all subject sessions from this endpoint.
- Rate-limit the endpoint by client ID plus trusted API Gateway source identity.
- Do not write durable audit events for obviously missing or random refresh tokens unless a separate coarse, rate-limited security event is intentionally added.

Admin account lifecycle routes can revoke all refresh tokens for a subject. The user-facing revoke endpoint only handles the session represented by the submitted refresh token.

## Access Token Boundary

Access tokens are self-contained JWTs and are not persisted. This endpoint does not revoke already-issued access tokens. Those remain valid until `exp` when a resource API validates JWTs locally.

## Security Invariants

- Raw refresh tokens are never stored in DynamoDB keys, logs, or errors.
- Responses do not reveal whether a refresh token exists.
- A client cannot revoke another client's refresh token.
- Revocation is idempotent.
- Random invalid tokens cannot create unbounded audit records.
- Refresh token reuse detection remains enforced on subsequent refresh attempts.

## Store Operations

- `revoke_refresh_token_family`
