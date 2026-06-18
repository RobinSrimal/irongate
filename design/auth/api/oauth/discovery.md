# Discovery And JWKS

Target code: `packages/functions/auth/src/api/oauth/discovery.rs`

## Owns

- OAuth authorization server metadata.
- OpenID Connect provider metadata.
- JWKS response for public verification keys.

## Target Endpoints

```text
GET /.well-known/openid-configuration
GET /.well-known/oauth-authorization-server
GET /.well-known/jwks.json
```

The OpenID Connect metadata endpoint is required for compatibility with generic OIDC clients. The OAuth authorization server metadata endpoint can expose the OAuth subset for clients that do not need OIDC.

## Metadata Requirements

OIDC metadata should advertise only implemented behavior:

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
claims_supported = ["sub", "iss", "aud", "exp", "iat", "nonce", "email", "email_verified"]
token_endpoint_auth_methods_supported = ["none", "client_secret_basic"]
code_challenge_methods_supported includes "S256"
```

Metadata must not advertise an introspection endpoint in v1. Access tokens are self-contained JWTs, and resource APIs validate them locally with JWKS metadata.

Metadata may advertise the revocation endpoint because v1 supports refresh-token revocation for logout. Revocation applies to refresh-token state, not self-contained access JWTs.

The practical v1 signing algorithm is ES256 because it matches the Rust/AWS template direction. Strict OpenID Provider certification has additional algorithm requirements, including RS256 support. If certification becomes a product goal, add a signing-algorithm design before implementation.

Discovery describes the Irongate auth server, not optional examples. It should not advertise hosted login pages, example frontend URLs, Cloudflare URLs, mobile app schemes, or desktop loopback helper endpoints as protocol capabilities.

## Security Invariants

- Metadata issuer must match the configured public issuer URL.
- JWKS must expose public key material only.
- Private signing keys must never be serialized through this endpoint.
- Algorithms should be explicit and narrow.
- Metadata must not advertise unsupported flows or algorithms.
- Metadata must not advertise unsupported token introspection.

## Store Operations

- `get_public_signing_keys` or KMS-backed equivalent metadata lookup.
