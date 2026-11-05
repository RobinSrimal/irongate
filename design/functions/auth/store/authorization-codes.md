# Authorization Codes

Implementation: [authorization-code operations](../src/store/authorization_codes.md) and
[provider issuance](../src/store/provider_issuance.md).

## Owns

- Short-lived OAuth authorization code records.
- HMAC lookup for authorization codes.
- Single-use code consumption.

## Behavior

Authorization codes are created after a provider proves identity and an authorize session is consumed.

Google/Apple callbacks prepare identity/account snapshots after token validation, then commit code
creation and identity work together. New or reusable deleted identities use four conditional writes:
account, identity, subject index and code. Returning identities use a complete active-account check,
exact identity replacement and absent-code Put. Preparation pins the existing account generation;
finish cannot reread a newer generation or choose a different subject. Password login keeps its
separate verified-password snapshot guard.

Provider preparation checks the consumed session's selected provider and logical expiry. Finish
rechecks expiry immediately before dispatch. The private session snapshot supplies client ID, exact
redirect URI, scope, downstream PKCE and OIDC nonce; the callback preserves client state in the
redirect. The route generates the raw code and passes only its HMAC digest into finishing.

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
prepare_provider_issuance
finish_provider_issuance
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

Existing-account issuance includes a complete active-account condition at the captured generation.
New provider accounts and their code are created together at generation zero. Code collision or
confirmed condition rejection leaves no partial issuance writes. There is one application-level
provider transaction attempt, not automatic re-resolution or upstream exchange retry.

Provider state/session consumes remain earlier and are not rolled back when issuance fails. An
operational failure, transaction conflict or lost response may leave the full issuance committed;
no compensation deletes the account or code. Unreturned codes expire normally. Code consumption,
JWT signing and response delivery remain separate operations. Neither expiry checks before dispatch
nor exact account guards establish an instantaneous revocation or lifecycle-history boundary.
