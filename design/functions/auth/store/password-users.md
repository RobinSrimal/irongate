# Password Users

Implementation: [store/password_users.rs](../src/store/password_users.md).

## What And Why

Password users are keyed by an HMAC of normalized email. Their value stores the normalized email,
PHC password hash, verification flag, optional generated subject and creation/update timestamps.
The raw password is not persisted.

## How

The store supports creation, lookup and conditional verification/hash replacement. Password
verification and OAuth continuation are domain behavior in [providers/password](../src/providers/password.md),
not a generic store login method.

Registration starts unverified. Email verification attaches an account subject after proof
consumption. Login requires that verified subject and active account status.
Reset replaces the hash after consuming a reset proof; those writes are not one transaction.
Account deletion replaces the password record with a tombstone that clears its email, subject,
password hash and hash timestamp while retaining lifecycle timestamps.

There is no automatic login-time hash upgrade or current-password-change endpoint.
