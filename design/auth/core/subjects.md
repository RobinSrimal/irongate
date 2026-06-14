# Subjects

Target code: `packages/functions/auth/src/core/subjects.rs`

## Owns

- Internal subject identifiers.
- Mapping verified provider identities to subjects.
- Rules for identity linking.

## Target Behavior

Subjects are generated stable identifiers that are stored in account and identity records.

```text
subject = user:<random-or-uuid-like-id>
```

Provider identity proofs map to a subject through persisted identity records:

```text
password verified email -> identity record -> subject
google issuer + sub -> identity record -> subject
apple issuer + sub -> identity record -> subject
```

The subject is stable after account creation, but it is not deterministically derived from email, issuer, or provider subject. This prevents account deletion followed by re-registration from accidentally receiving the same `sub`.

## Security Invariants

- Do not use email alone for Google or Apple identity.
- Do not auto-link different providers by matching email.
- Subject IDs must be stable, opaque, and non-reversible enough for token claims.
- Deleted accounts cannot be silently recreated with the same subject.
- Provider claims are inputs, not the source of all account truth.
