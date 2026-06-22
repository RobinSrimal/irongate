# Identities

Target code: `packages/functions/auth/src/core/identities.rs`

## Owns

- Provider identity model.
- Account linking rules.
- Internal subject mapping.

## Target Identity Types

Password identity:

```text
provider = "password"
key = normalized verified email
subject = generated persisted subject
```

Google identity:

```text
provider = "google"
key = issuer + sub
subject = generated persisted subject
```

Apple identity:

```text
provider = "apple"
key = issuer + sub
subject = generated persisted subject
```

## Linking Decision

The target core does not auto-link identities by email address.

If a user signs in with password and Google using the same email, those are separate identities.

## Persistence Decision

V1 persists a minimal identity record after identity proof succeeds:

- Password identity after email verification.
- Google identity after OIDC issuer, audience, nonce, and subject validation.
- Apple identity after OIDC issuer, audience, nonce, and subject validation.

The persisted record stores the provider, lookup digest, generated internal subject, optional contact metadata, `created_at`, and `last_seen_at`. It must not make email the primary key for OIDC identities.

Subjects are generated when the identity is first accepted and then persisted. They are not re-derived from email or provider claims on every login, because account deletion must not accidentally recreate the same `sub`.

## Security Invariants

- OIDC identity is based on `issuer + sub`, not email.
- Password identity requires verified email.
- Email is a claim or contact attribute, not a universal account key.
- Provider claims can change and should not be treated as permanent identity keys unless specified by the provider.
- Account linking requires explicit user proof for both identities.
- Persisted identity mappings cannot be silently reassigned to another subject.
- Deleted identity mappings cannot be silently recreated with the same subject.
