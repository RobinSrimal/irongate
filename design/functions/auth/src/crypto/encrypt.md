# Cookies And Encryption

Source: [packages/functions/auth/src/crypto/encrypt.rs](../../../../../packages/functions/auth/src/crypto/encrypt.rs).

## What

SecureCookie header construction plus standalone RSA-OAEP/AES-GCM helpers.

## Why

Cookie attributes and cryptographic serialization are centralized.

## How

The cookie builder defaults to HttpOnly, Secure, SameSite=Strict and Path=/. Encryption wraps a random AES key with RSA-OAEP and encodes three base64url parts. Constructing a SecureCookie does not itself encrypt its value; authorize currently uses an opaque session identifier.
