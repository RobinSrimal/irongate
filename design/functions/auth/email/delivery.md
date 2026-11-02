# Resend Email Delivery

Implementation: [email/mod.rs](../src/email/mod.md) and [email/resend.rs](../src/email/resend.md).

## What And Why

One Resend transport sends rendered verification and reset emails. The
`VerificationEmailSender` trait lets domain tests capture messages without contacting a provider;
despite its name, both action types use it.

## How

The sender receives the configured API key, sender/reply-to and rendered subject/HTML/text.
It posts to Resend and requires a successful response containing a delivery ID. Transport/status
errors do not themselves verify an account or change a password.

Dev and production both require real Resend configuration. Sender, branding and action URL bases
are non-secret stage settings; the API key is an SST secret. Built-in rendering behavior and the
unused template-path fields are described in [templates](templates.md).
