# Provider States

Implementation: [provider-state operations](../src/store/provider_states.md).

## Owns

- External provider callback state.
- HMAC lookup for provider state.
- Single-use provider state consumption.

## Behavior

Provider state ties an external Google or Apple callback to an internal authorize session.

The callback consumes state and then its authorize session before exchange and signed-token
validation. Those consumes remain outside the later atomic [provider issuance](identities.md)
transaction. Provider/validation/storage failure can therefore leave them consumed; the user must
restart sign-in. Issuance does not retry upstream exchange, recreate state or compensate by deleting
an account after an uncertain response.

The raw provider state is sent to the external provider once. DynamoDB stores only a lookup digest:

```text
provider_state_lookup_digest = hex(HMAC-SHA256(storage_lookup_secret, "provider_state" || NUL || state))
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
