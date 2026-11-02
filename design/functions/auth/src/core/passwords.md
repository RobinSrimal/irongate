# Password Policy

Source: [packages/functions/auth/src/core/passwords.rs](../../../../../packages/functions/auth/src/core/passwords.rs).

## What

Email normalization, password length validation and hashing entrypoint.

## Why

Registration and reset share policy without embedding HTTP or persistence concerns.

## How

Email is trimmed, lowercased and validated. Password length limits are checked before calling the Argon2 hashing helper; failures use typed PasswordError values.
