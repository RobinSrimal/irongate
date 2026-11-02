# Basic Authentication

Source: [packages/functions/auth/src/client/mod.rs](../../../../../packages/functions/auth/src/client/mod.rs).

## What

Parsing client credentials from an HTTP Basic header.

## Why

Token and revocation endpoints share credential decoding.

## How

Requires the Basic prefix, valid base64/UTF-8 and nonempty ID/secret. Splitting on the first colon preserves colons inside the secret. Inline tests exercise valid credentials and malformed inputs; registry authentication is separate.
