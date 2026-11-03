# Authorization Codes

Implementation: `packages/functions/auth/src/store/authorization_codes.rs`

## Owns

- Short-lived OAuth authorization code records.
- HMAC lookup for authorization codes.
- Single-use code consumption.

## Behavior

Authorization codes are created after a provider proves identity and an authorize session is consumed.

The raw code is returned to the browser redirect once. DynamoDB stores only a lookup digest:

```text
auth_code_lookup_digest = hex(HMAC-SHA256(storage_lookup_secret, "authorization_code" || NUL || code))
```

Record shape:

```json
{
  "client_id": "...",
  "redirect_uri": "...",
  "subject": "...",
  "subject_type": "user",
  "credential_generation": 0,
  "properties": {},
  "code_challenge": "...",
  "scope": "...",
  "oidc_nonce": "optional",
  "created_at": "...",
  "expires_at": "..."
}
```

The expiry is derived from `AUTH_AUTH_CODE_TTL_SECONDS` and is written both inside the record and as the DynamoDB `expiry` attribute.

## Store Operations

```text
create_authorization_code
create_password_authorization_code
take_authorization_code
get_authorization_code
delete_authorization_code_if_current
```

## Security Invariants

- Raw authorization codes never appear in `pk`, `sk`, logs, or errors.
- Codes are short-lived.
- Codes are single-use.
- The token handler checks client ID, redirect URI and PKCE before conditional deletion. The store
  checks runtime expiry and generation, guards the active account and compares the complete code;
  its generic take method does not itself
  authenticate the client or validate a verifier.
- Consuming a code returns the stored OIDC nonce for initial ID-token issuance.
- Expired records are rejected even if DynamoDB TTL has not deleted them.

Creation includes a complete active-account condition at the captured generation. Password login
also guards the exact password-user snapshot verified by the domain. A stale credential is never
upgraded to the latest generation. Code consumption and later signing are separate operations.
