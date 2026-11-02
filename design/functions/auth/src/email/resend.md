# Resend Delivery

Source: [packages/functions/auth/src/email/resend.rs](../../../../../packages/functions/auth/src/email/resend.rs).

## What

The deployed email sender and Resend request construction.

## Why

Verification and recovery use one external delivery integration.

## How

A reused reqwest client posts the rendered message with bearer API-key authentication. Transport and non-success responses become delivery errors. with_endpoint supports local testing; successful delivery does not authenticate or verify a user.
