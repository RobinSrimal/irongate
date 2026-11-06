# Email Boundary

Source: [packages/functions/auth/src/email/mod.rs](../../../../../packages/functions/auth/src/email/mod.rs).

## What

RenderedEmail, delivery errors and the VerificationEmailSender interface.

## Why

Password flows and internal Google confirmation can share delivery while tests capture messages
without Resend.

## How

Exports password and confirmation renderers and the Resend implementation. The existing async sender
accepts rendered HTML/text and recipient information; a no-op implementation is compiled for unit
tests. Its name does not restrict it to registration emails. No additional transport or secret is
introduced for the internal sign-in code.
