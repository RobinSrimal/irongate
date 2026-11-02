# Discovery And JWKS

Implementation: `packages/functions/auth/src/api/oauth/discovery.rs`

## Owns

- OAuth authorization server metadata.
- OpenID Connect provider metadata.
- JWKS response for public verification keys.

## Endpoints

```text
GET /.well-known/openid-configuration
GET /.well-known/oauth-authorization-server
GET /.well-known/jwks.json
```

Both discovery endpoints currently return the same metadata object. The implementation lives in
`src/oauth/well_known.rs`; the API module re-exports it.

## Metadata

The current response advertises:

```text
issuer
authorization_endpoint
token_endpoint
userinfo_endpoint
revocation_endpoint
jwks_uri
response_types_supported = ["code"]
grant_types_supported = ["authorization_code", "refresh_token"]
subject_types_supported = ["public"]
id_token_signing_alg_values_supported = ["ES256"]
scopes_supported = ["openid", "profile", "email", "offline_access"]
claims_supported = ["sub", "iss", "aud", "exp", "iat", "email", "email_verified"]
token_endpoint_auth_methods_supported = ["none", "client_secret_post", "client_secret_basic"]
code_challenge_methods_supported includes "S256"
```

Metadata advertises the implemented OAuth/OIDC endpoints and JWKS. Access tokens are
self-contained JWTs, and resource APIs validate them locally with JWKS metadata.

Metadata advertises the mounted refresh-token revocation endpoint. Revocation does not invalidate
self-contained access JWTs.

Only ES256 is implemented for first-party signing. This repository does not claim OpenID Provider
certification. ID tokens can carry a supplied nonce even though that field is absent from the
current advertised claim list.

Discovery describes the Irongate auth server, not optional examples. It should not advertise hosted login pages, example frontend URLs, Cloudflare URLs, mobile app schemes, or desktop loopback helper endpoints as protocol capabilities.

## Security Invariants

- Metadata issuer must match the configured public issuer URL.
- JWKS must expose public key material only.
- Private signing keys must never be serialized through this endpoint.
- Algorithms should be explicit and narrow.
- Metadata must not advertise unsupported flows or algorithms.

## Key Source

JWKS comes directly from `runtime.signer.jwks()`, not a DynamoDB read. It exposes one configured
public key. The current JWK serializer uses `use_` instead of the standard `use` field, as noted in
[signing](../../crypto/signing.md).
