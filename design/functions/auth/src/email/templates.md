# Email Rendering

Source: [packages/functions/auth/src/email/templates.rs](../../../../../packages/functions/auth/src/email/templates.rs).

## What

Built-in verification, reset and internal Google sign-in confirmation HTML/text messages.

## Why

Message construction keeps proofs and escaping consistent without involving hosted pages.

## How

The renderer appends a URL-encoded token query parameter to configured URL bases, escapes HTML
values and applies configured brand, support address and subjects. Verification messages ask the
recipient to choose a password and complete account setup; opening a link alone does not activate
an account. The app owns the completion form, including the existing optional web example's
non-mutating GET and deliberate POST. No core-hosted HTML page is introduced.

Reset wording and URL handling remain separate and unchanged. Configured template-path fields are
not read by this renderer. See [email setup](../../../../../docs/setup/06-resend-email.md).

The confirmation renderer takes the generated code and actual pending expiry, preserving leading
zeros and an exact RFC3339 deadline in HTML/text. Configured brand/support values are HTML-escaped;
the subject is the brand plus `sign-in code`. Copy directs entry only in the original sign-in window
and ignoring unsolicited messages. It generates no action link. There is no separate confirmation
template override or configuration field; inline tests cover proof formatting, deadline and escaping.
