# Password Hashing

Implementation: [crypto/password.rs](../src/crypto/password.md) and
[core/passwords.rs](../src/core/passwords.md).

## What And Why

Passwords use Argon2id with a fresh OS-random salt. The stored PHC string carries algorithm,
version, parameters, salt and hash, so verification does not need a separate salt record.

## How

The crypto module uses `Argon2::default()` for hashing and the library's PHC verification path.
Malformed stored hashes and mismatches return false. Password bytes are hashed exactly as submitted;
email normalization is separate.

The policy defaults to 12 through 128 Unicode scalar values, with no character-class requirements.
The length validator accepts a policy value; the storage-hashing helper also applies its default
policy. There is no implemented parameter-upgrade inspection or automatic login-time rehash.

Hashes remain sensitive data and are not returned in user or admin API responses.
