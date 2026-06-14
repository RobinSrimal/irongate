# Subjects

Target code: `packages/functions/auth/src/core/subjects.rs`

## Owns

- Internal subject identifiers.
- Mapping verified provider identities to subjects.
- Rules for identity linking.

## Target Behavior

Email identity:

```text
subject = hash("email", normalized verified email)
```

OIDC identity:

```text
subject = hash("oidc", issuer, sub)
```

## Security Invariants

- Do not use email alone for Google or Apple identity.
- Do not auto-link different providers by matching email.
- Subject IDs must be stable and non-reversible enough for token claims.
- Provider claims are inputs, not the source of all account truth.
