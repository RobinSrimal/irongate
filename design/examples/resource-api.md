# Resource API Example

Target code: `packages/examples/resource-api`

## Owns

- Minimal protected API.
- Access-token verification.
- Scope and audience checks.
- Example row-level access-control boundary using `sub`.

## Must Not Own

- User login.
- Token issuance.
- Refresh-token storage.
- Token introspection.
- Raw auth table access.

## Access Token Validation

The API validates Irongate access JWTs locally:

```text
issuer
audience
expiry
signature
algorithm
key ID
scope
subject
```

JWKS comes from the Irongate issuer metadata.

## Row-Level Access Control

The example may use the access-token `sub` claim as the stable user/account identifier for user-owned data.

Rules:

- Do not trust an unverified token body.
- Validate the signature and issuer before reading claims.
- Validate audience before accepting the token for this API.
- Use scopes or explicit authorization claims for operations.
- Treat `sub` as an opaque stable identifier, not an email address.

## Security Invariants

- No token introspection dependency in v1.
- No raw `AuthTable` reads.
- No refresh tokens accepted by resource APIs.
- No ID tokens accepted as API authorization tokens.
- Already-issued access tokens remain valid until expiry unless the API adds its own revocation cache.
