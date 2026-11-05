# Email Rendering

Source: [packages/functions/auth/src/email/templates.rs](../../../../../packages/functions/auth/src/email/templates.rs).

## What

Built-in verification and reset HTML/text messages.

## Why

Message construction keeps bearer links and escaping consistent across password flows.

## How

The renderer appends a URL-encoded token query parameter to configured URL bases, escapes HTML
values and applies configured brand, support address and subjects. Verification messages ask the
recipient to choose a password and complete account setup; opening a link alone does not activate
an account. The app owns the completion form, including the existing optional web example's
non-mutating GET and deliberate POST. No core-hosted HTML page is introduced.

Reset wording and URL handling remain separate and unchanged. Configured template-path fields are
not read by this renderer. See [email setup](../../../../../docs/setup/06-resend-email.md).
