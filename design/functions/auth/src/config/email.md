# Email Configuration

Source: [packages/functions/auth/src/config/email.rs](../../../../../packages/functions/auth/src/config/email.rs).

## What

Resend credentials, sender, link bases, brand, subjects and optional template-path fields.

## Why

Email-specific values are explicit configuration rather than provider-route constants.

## How

Required values and URL syntax are checked and the API key is redacted in Debug. Template paths are retained in configuration, but the current renderer uses built-in HTML/text and does not load those files.
