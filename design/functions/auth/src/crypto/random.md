# Random Values

Source: [packages/functions/auth/src/crypto/random.rs](../../../../../packages/functions/auth/src/crypto/random.rs).

## What

OS-backed random strings, bytes, numeric strings and UUIDs.

## Why

Codes, session identifiers and other generated values need unpredictable inputs.

## How

String selection uses rand's uniform range API over the base64url alphabet. Numeric strings use rejection sampling; raw bytes use OsRng. Presence of a numeric helper does not imply an OTP endpoint is mounted.
