# Refresh Token Store

Target code: `packages/functions/auth/src/store/refresh.rs`

## Owns

- Refresh token record creation.
- Atomic refresh rotation.
- Reuse detection.
- User-facing refresh token family revocation.
- Subject/client revocation indexes.

## Target Records

```text
refresh:<digest>
refresh_by_subject:<subject> / <digest>
refresh_by_client:<client_id> / <digest>
```

All records use the refresh token expiry as TTL. The expiry is derived from `AUTH_REFRESH_TOKEN_TTL_SECONDS` and is written both inside the record and as the DynamoDB `expiry` attribute.

Refresh token records should retain the original granted scopes and whether the original authentication was an OIDC flow. If refresh responses issue ID tokens, they use this metadata to preserve `iss`, `sub`, and `aud` correctly.

## Store Operations

```text
create_refresh_token
rotate_refresh_token
revoke_refresh_token_family
revoke_refresh_tokens_for_subject
```

`revoke_refresh_token_family` is used by user-facing logout. It revokes the current refresh token family/session for the submitted token without scanning the table.

`revoke_refresh_tokens_for_subject` is used by IAM-protected account lifecycle operations. It revokes all refresh tokens for the subject using bounded subject index queries.

## Security Invariants

- Refresh token lookup uses HMAC digest.
- Refresh token creation writes the token record, family record, and subject/client index records in one transaction.
- Rotation is atomic.
- The old token is marked replaced when the new token is created.
- Reuse of a replaced or revoked token is detected.
- Revocation does not scan the entire refresh-token family.
- User-facing revocation does not reveal whether a refresh token exists.
- User-facing revocation cannot revoke another client's refresh token.
- Refresh-token issuance requires both client permission and the configured offline-access/session policy.
- Account disable/delete paths revoke refresh tokens for the subject without scanning the full table.
