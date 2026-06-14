# Provider States

Target code: `packages/functions/auth/src/store/provider_states.rs`

## Owns

- External provider callback state.
- HMAC lookup for provider state.
- Single-use provider state consumption.

## Target Behavior

Provider state ties an external Google or Apple callback to an internal authorize session.

The raw provider state is sent to the external provider once. DynamoDB stores only a lookup digest:

```text
provider_state_lookup_digest = HMAC-SHA256(storage_lookup_secret, "provider_state:" || state)
```

Record shape:

```json
{
  "session_lookup_digest": "...",
  "provider": "google",
  "pkce_verifier": "...",
  "nonce": "...",
  "created_at": "...",
  "expires_at": "..."
}
```

The expiry is derived from `AUTH_PROVIDER_STATE_TTL_SECONDS` and is written both inside the record and as the DynamoDB `expiry` attribute.

## Store Operations

```text
create_provider_state
take_provider_state
```

## Security Invariants

- Raw provider state never appears in `pk`, `sk`, logs, or errors.
- Provider state is short-lived.
- Provider state is single-use.
- Consuming provider state verifies expiry before returning callback metadata.
- Provider state records are not operator-safe.
