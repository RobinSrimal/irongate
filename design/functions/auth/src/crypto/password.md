# Password Hashing

Source: [packages/functions/auth/src/crypto/password.rs](../../../../../packages/functions/auth/src/crypto/password.rs).

## What

Argon2 password hashing and PHC-string verification.

## Why

A unique salt prevents identical passwords from producing identical stored hashes.

## How

Each hash uses an OS-random salt and Argon2 defaults. The returned PHC string contains algorithm parameters, salt and hash; verification parses that string and rejects malformed hashes.
