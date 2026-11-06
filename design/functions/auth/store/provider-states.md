# Provider States

Implementation: [provider-state operations](../src/store/provider_states.md).

## Owns

- External provider callback state.
- HMAC lookup for provider state.
- Single-use provider state consumption.

## Behavior

Provider state ties an external Google or Apple callback to an internal authorize session.

Google state additionally binds an independent browser proof. The typed consumer requires a
Google browser HMAC or an explicit Apple expectation. Wrong proof/provider cannot consume the record.
Only confirmed state consumption permits consuming the authorize session before exchange and signed-token
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
  "browser_digest": "...",
  "pkce_verifier": "...",
  "nonce": "...",
  "created_at": "...",
  "expires_at": "..."
}
```

Google requires `browser_digest`; Apple uses `provider=apple` without a Google browser field.
There is no unbound Google variant. Browser HMAC uses `LookupFamily::SignInBrowser`; only the
issuer-host cookie holds its raw proof. The upstream verifier and nonce remain server-side state.

Expiry derives from `AUTH_PROVIDER_STATE_TTL_SECONDS`, capped for Google at the original authorize
session deadline, and is written inside the record and as DynamoDB `expiry`. Creation rejects invalid
digests/records and uses an absent-key condition. Consumption strongly reads the exact key, checks
provider/proof/expiry, compares browser HMACs in constant time and deletes the original raw value
conditionally, rechecking expiry before dispatch. It neither rereads nor retries a replacement.
Confirmed conditional loss returns no record; operational/uncertain failures propagate. Expired
records may remain until TTL cleanup. A failed response can follow committed deletion.

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
- Google browser proof is required even for provider-declined callbacks; malformed stored bindings
  fail closed, and existing unbound Google transactions require a fresh sign-in.
- Provider state records are not operator-safe.
