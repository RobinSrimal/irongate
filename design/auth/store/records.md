# Store Records

Target code: `packages/functions/auth/src/store/records.rs`

## Owns

- Typed structs for stored records.
- Versioning if record shapes change.
- Timestamp and expiry fields.

## Target Record Families

| Family | Secret-bearing? | Operator-safe? | Notes |
| --- | --- | --- | --- |
| Authorize session | Yes | No | Contains redirect, state, scope, PKCE challenge, and flow state. |
| Provider state | Yes | No | Contains session reference and may contain PKCE verifier/nonce. |
| Authorization code | Yes | No | Contains subject and code exchange metadata. |
| Refresh token | Yes | No | Contains token family and revocation metadata. |
| Password user | Yes | No | Contains email and password hash. |
| Email verification link token | Yes | No | Contains verification metadata; lookup key is HMAC. |
| Password reset link token | Yes | No | Contains reset metadata; lookup key is HMAC. |
| Identity mapping | Sensitive | No | Persisted for password, Google, and Apple; stores provider, digest, subject, optional contact metadata, and timestamps. |
| Account | No bearer secrets | Sanitized status only | Stores subject lifecycle status and timestamps. |
| Rate-limit counter | Low | No | Can contain source or email-derived identifiers. |
| Signing key reference or metadata | High | No | Public metadata is safe; private material is not. |

OAuth clients are config-only in v1 and are not ordinary auth-table records. The source of truth is deployment configuration, not DynamoDB.

## Security Invariants

- Stored records should not contain raw bearer tokens or raw link tokens.
- Sensitive values that must be stored should be hashes, HMACs, or encrypted material.
- Records should carry enough expiry metadata for runtime checks.
- Metrics/audit projections must be explicitly sanitized before operator use.
