# Store Records

Target code: `packages/functions/auth/src/store/records.rs`

## Owns

- Typed structs for stored records.
- Versioning if record shapes change.
- Timestamp and expiry fields.

## Target Record Families

| Family | Secret-bearing? | Operator-safe? | Notes |
| --- | --- | --- | --- |
| OAuth client | Sometimes | No | Confidential clients include secret hashes and security config. |
| Authorize session | Yes | No | Contains redirect, state, scope, PKCE challenge, and flow state. |
| Provider state | Yes | No | Contains session reference and may contain PKCE verifier/nonce. |
| Authorization code | Yes | No | Contains subject and code exchange metadata. |
| Refresh token | Yes | No | Contains token family and revocation metadata. |
| Password user | Yes | No | Contains email and password hash. |
| Email verification code or link | Yes | No | Contains verification metadata; lookup key is HMAC. |
| Password reset code or link | Yes | No | Contains reset metadata; lookup key is HMAC. |
| Identity mapping | Sensitive | Maybe | Operator-safe only if stripped of raw email/provider claims. |
| Rate-limit counter | Low | No | Can contain source or email-derived identifiers. |
| Signing key reference or metadata | High | No | Public metadata is safe; private material is not. |

## Security Invariants

- Stored records should not contain raw bearer tokens or raw short codes.
- Sensitive values that must be stored should be hashes, HMACs, or encrypted material.
- Records should carry enough expiry metadata for runtime checks.
- Metrics/audit projections must be explicitly sanitized before operator use.
