# Authorize Sessions

Implementation: [authorize-session operations](../src/store/authorize_sessions.md).

## Owns

- Short-lived OAuth authorize session records.
- HMAC lookup for browser authorize session keys.
- Single-use session consumption for password login and external-provider callbacks.

## Behavior

`GET /authorize` creates an authorize session after validating the OAuth client, redirect URI, requested scope, and PKCE parameters.

The raw session key is sent only to the browser/provider flow. DynamoDB stores only a lookup digest:

```text
session_lookup_digest = hex(HMAC-SHA256(storage_lookup_secret, "oauth_session" || NUL || session_key))
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

Google/Apple callbacks consume provider state and then the authorize session before upstream code
exchange and token validation. [Provider issuance](identities.md) later receives that consumed
snapshot, validates selected-provider match and expiry during preparation, and rechecks expiry
immediately before its identity/account/code transaction dispatch. Password login consumes its
session after checking the password, verified user and active account.

The consumes and external exchange are not part of provider issuance's transaction. A later failure
does not restore them: restart sign-in instead of replaying a callback. Pre-dispatch expiry checks
do not promise zero-time network/storage processing; TTL deletion is not authorization.

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
