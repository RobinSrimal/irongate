# Auth Crypto

Target code: `packages/functions/auth/src/crypto`

## Owns

- Token signing primitives.
- Password hashing primitives.
- HMAC lookup digests.
- Secure cookie helpers.
- Random generation helpers.

## Must Not Own

- DynamoDB calls.
- Provider HTTP requests.
- OAuth route handling.

## Security Invariants

- Use narrow algorithms.
- Keep key material out of logs.
- Keep random values high entropy.
- Prefer non-exportable signing keys when feasible.

## Design Files

- `hmac-lookups.md`: lookup digests for bearer-style secrets.
- `passwords.md`: Argon2id password hashing and password policy.
- `signing.md`: JWT signing modes and JWKS.
