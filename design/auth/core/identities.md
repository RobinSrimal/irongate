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
subject = user:<hash("password", normalized_email)>
```

Google identity:

```text
provider = "google"
key = issuer + sub
subject = user:<hash("oidc", issuer, sub)>
```

Apple identity:

```text
provider = "apple"
key = issuer + sub
subject = user:<hash("oidc", issuer, sub)>
```

## Linking Decision

The target core does not auto-link identities by email address.

If a user signs in with password and Google using the same email, those are separate identities unless a later explicit linking flow is designed.

## Security Invariants

- OIDC identity is based on `issuer + sub`, not email.
- Password identity requires verified email.
- Email is a claim or contact attribute, not a universal account key.
- Provider claims can change and should not be treated as permanent identity keys unless specified by the provider.
- Account linking requires explicit user proof for both identities and is postponed.
