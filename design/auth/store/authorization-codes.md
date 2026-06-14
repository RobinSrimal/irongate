# Authorization Codes

Target code: `packages/functions/auth/src/store/authorization_codes.rs`

## Owns

- Short-lived OAuth authorization code records.
- HMAC lookup for authorization codes.
- Single-use code consumption.

## Target Behavior

Authorization codes are created after a provider proves identity and an authorize session is consumed.

The raw code is returned to the browser redirect once. DynamoDB stores only a lookup digest:

```text
auth_code_lookup_digest = HMAC-SHA256(storage_lookup_secret, "auth_code:" || code)
```

Record shape:

```json
{
  "client_id": "...",
  "redirect_uri": "...",
  "subject": "...",
  "subject_type": "user",
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
take_authorization_code
```

## Security Invariants

- Raw authorization codes never appear in `pk`, `sk`, logs, or errors.
- Codes are short-lived.
- Codes are single-use.
- Consuming a code checks client ID, redirect URI, expiry, and PKCE data.
- Consuming a code returns the stored OIDC nonce for initial ID-token issuance.
- Expired records are rejected even if DynamoDB TTL has not deleted them.
