# Identity Store

Target code: `packages/functions/auth/src/store/identities.rs`

## Owns

- Optional persisted identity records.
- First-seen and last-seen metadata.
- Provider identity to internal subject mapping.

## Decision

The auth core can derive subjects statelessly from verified identity inputs, but a persisted identity record is useful for audit and account lifecycle.

For v1, identity persistence should be minimal and non-secret.

## Target Records

```text
identity:<provider>:<identity_digest>
```

Value:

```json
{
  "provider": "google",
  "identity_digest": "...",
  "subject": "user:...",
  "email": "optional",
  "email_verified": true,
  "created_at": "...",
  "last_seen_at": "..."
}
```

## Security Invariants

- Raw Google/Apple `sub` values should not be used directly in keys.
- Email is optional metadata, not an identity key for OIDC providers.
- No automatic linking by matching email.

## Open Decision

We need to decide before implementation whether v1 persists identity records or derives subjects only from provider proof. If persisted, keep records minimal and treat them as sensitive.
