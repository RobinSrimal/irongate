# Authorize Sessions

Target code: `packages/functions/auth/src/store/authorize_sessions.rs`

## Owns

- Short-lived OAuth authorize session records.
- HMAC lookup for browser authorize session keys.
- Single-use session consumption after provider proof succeeds.

## Target Behavior

`GET /authorize` creates an authorize session after validating the OAuth client, redirect URI, requested scope, and PKCE parameters.

The raw session key is sent only to the browser/provider flow. DynamoDB stores only a lookup digest:

```text
session_lookup_digest = HMAC-SHA256(storage_lookup_secret, "authorize_session:" || session_key)
```

Record shape:

```json
{
  "client_id": "...",
  "redirect_uri": "...",
  "state": "...",
  "scope": "...",
  "oidc_nonce": "optional",
  "code_challenge": "...",
  "code_challenge_method": "S256",
  "selected_provider": "optional",
  "created_at": "...",
  "expires_at": "..."
}
```

The expiry is derived from `AUTH_AUTHORIZE_SESSION_TTL_SECONDS` and is written both inside the record and as the DynamoDB `expiry` attribute.

## Store Operations

```text
create_authorize_session
take_authorize_session
```

## Security Invariants

- Raw authorize session keys never appear in `pk`, `sk`, logs, or errors.
- Authorize sessions are short-lived.
- Authorize sessions are consumed once.
- Consuming a session verifies expiry before returning callback metadata.
- OIDC client nonce is stored separately from Google/Apple provider nonce.
- Expired records are rejected even if DynamoDB TTL has not deleted them.
