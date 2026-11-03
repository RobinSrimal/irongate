# Store Records

Implementation: [store/records.rs](../src/store/records.md); refresh, password and rate-limit modules
also define their own record types.

## What And Why

Typed records preserve protocol bindings, lifecycle timestamps and runtime expiry metadata instead
of exposing arbitrary DynamoDB values to routes. They serialize into the table's JSON `value`
attribute; short-lived writes also carry a numeric `expiry` attribute.

## Record Families

- Authorize sessions: client, redirect, state, scopes, PKCE, provider selection and OIDC nonce.
- Provider state: upstream nonce/verifier and original authorize-session continuation.
- Authorization codes: subject, properties, client/redirect, scope and PKCE bindings.
- Refresh tokens/families: subject/client association, replacement and revocation state.
- Password users and proofs: normalized contact metadata, PHC hash, verification/reset state.
- Accounts and identities: stable subjects, provider mappings, status and timestamps.
- Rate-limit counters: count and window start.

Raw authorization codes, refresh tokens and email action tokens are not stored as lookup keys.
However, provider-state values contain a session lookup digest and the raw upstream PKCE verifier.
The record-value boundary is therefore sensitive, not sanitized simply because keys use HMAC.

OAuth clients are loaded from configuration, and signing private keys are supplied outside this
table. The current record types have no general schema-version or migration framework.

Accounts, reset proofs, authorization codes, refresh tokens and families require a `u64`
credential_generation. Missing or malformed counters fail closed; there is no default-zero reader
or backfill. The account starts at zero and recovery advances it. Bound credentials keep their
original generation. Existing populated deployments require an explicit data decision before rollout.
