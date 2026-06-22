# Password Hashing

Target code: `packages/functions/auth/src/crypto/passwords.rs`

## Owns

- Password hashing.
- Password hash verification.
- Password policy validation.
- Password hash parameter/version inspection.
- Rehash recommendation when stored parameters are stale.

## Password Policy

V1 policy:

```text
min length: 12 characters
max length: 128 characters
composition rules: none
```

Do not require character classes like uppercase, lowercase, digit, or symbol. Length is the main built-in rule.

Email normalization is required. Password normalization is not applied in v1; validate length on the submitted password string and hash the exact submitted password bytes.

## Argon2id

Use Argon2id for password hashing.

The stored password hash should be a PHC string containing algorithm, version, salt, parameters, and hash output.

Target default parameters should be explicit in code and versioned in config/tests. The exact numeric parameters should be chosen during implementation by measuring Lambda latency and memory, but the design requirements are:

- Use Argon2id, not Argon2i or Argon2d.
- Use a unique random salt per password.
- Store the full PHC string.
- Keep enough metadata to decide whether a hash should be upgraded.
- Rehash on successful login or password reset when stored parameters are below current policy.

## Policy Shape

The built-in policy is length-based. It avoids composition rules, password history, and forced
periodic rotation so the template has one predictable password policy.

## Security Invariants

- Raw passwords are never logged.
- Password hashes are never returned through APIs.
- Password hashes are treated as sensitive raw auth data.
- Verification should use the password-hashing library's constant-time verification path.
- Hash errors should not reveal whether the email exists.
