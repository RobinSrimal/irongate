# Identity Store

Target code: `packages/functions/auth/src/store/identities.rs`

## Owns

- Persisted identity records.
- First-seen and last-seen metadata.
- Provider identity to internal subject mapping.

## Decision

V1 persists minimal identity records for password, Google, and Apple identities.

The auth core derives the deterministic identity lookup from verified proof, but the store records the mapping to a generated internal subject plus first-seen and last-seen metadata. This gives the system stable account lifecycle state without making email the universal identity key.

## Target Records

```text
identity:<provider>:<identity_digest>
```

Value:

```json
{
  "provider": "google",
  "identity_digest": "...",
  "subject": "user:generated...",
  "status": "active",
  "email": "optional",
  "email_verified": true,
  "created_at": "...",
  "last_seen_at": "...",
  "deleted_at": "optional",
  "reuse_after": "optional"
}
```

Active identity records may store optional contact metadata. Deleted identity tombstones must strip contact metadata and retain only the fields needed for reuse policy:

```json
{
  "provider": "google",
  "identity_digest": "...",
  "status": "deleted",
  "deleted_at": "...",
  "reuse_after": "optional"
}
```

Password identities should also use this identity family after email verification succeeds:

```text
provider = "password"
identity_digest = HMAC-SHA256(storage_lookup_secret, normalized_verified_email)
```

Google and Apple identities should derive `identity_digest` from issuer plus provider subject:

```text
identity_digest = HMAC-SHA256(storage_lookup_secret, issuer + "\n" + sub)
```

## Store Operations

```text
get_identity(provider, identity_digest)
create_identity_from_verified_proof(provider, identity_digest, generated_subject, metadata)
touch_identity_last_seen(provider, identity_digest)
mark_identity_deleted(provider, identity_digest)
reuse_deleted_identity_with_new_subject(provider, identity_digest, generated_subject, metadata)
```

The create operation should be idempotent for the same active identity and subject. If an existing active identity maps to a different subject, the operation fails and logs a security event.

For a first-time verified identity, account creation and identity creation should be transactional: create the account record with a generated subject and create the identity mapping to that subject together.

Deleted identity mappings must not silently recreate the same subject. Reuse is controlled by `AUTH_DELETED_IDENTITY_REUSE` and `AUTH_DELETED_IDENTITY_RETENTION_DAYS`.

When an identity is deleted, the store should preserve a tombstone with `status=deleted`, `deleted_at`, and optional `reuse_after`. The tombstone stores the HMAC identity digest, not raw email or provider subject data.

The tombstone must not retain email, email verification flags, profile claims, provider claims, or the old subject. The old subject remains only on the account tombstone.

When the configured policy allows reuse, the store may replace the deleted mapping with a new active mapping to a newly generated subject. That transition must be conditional on the deleted state and reuse eligibility, and it must not reuse the previous subject.

## Security Invariants

- Raw Google/Apple `sub` values should not be used directly in keys.
- Email is optional metadata, not an identity key for OIDC providers.
- No automatic linking by matching email.
- Identity records are sensitive and are not operator-safe raw data.
- Existing identity mappings cannot be silently overwritten to a different subject.
- Deleted identity mappings cannot be silently reused outside the configured deleted identity reuse policy.
- Deleted identity tombstones do not contain contact metadata or old subject links.
- Reused deleted identities always map to a newly generated subject.
