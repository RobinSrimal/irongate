# Web Randomness

Source: [packages/examples/web/src/crypto.ts](../../../../packages/examples/web/src/crypto.ts).

## What

Random state/verifier strings, session IDs and SHA-256 base64url helpers.

## Why

OAuth transaction values and cookies require fresh unpredictable inputs.

## How

Web Crypto supplies random bytes and digests. Session IDs encode 32 random bytes; randomString maps bytes modulo a 66-character alphabet, so its character distribution is not perfectly uniform.
