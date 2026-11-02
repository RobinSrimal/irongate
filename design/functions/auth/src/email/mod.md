# Email Boundary

Source: [packages/functions/auth/src/email/mod.rs](../../../../../packages/functions/auth/src/email/mod.rs).

## What

RenderedEmail, delivery errors and the VerificationEmailSender interface.

## Why

Password flows can render/send through one boundary while tests capture messages without Resend.

## How

Exports the renderer and Resend implementation. The async sender accepts rendered HTML/text and recipient information; a no-op implementation is compiled for unit tests.
