# Email Templates

## What

Built-in HTML/text verification and reset messages in [the renderer](../src/email/templates.md).

## Why

The password domain passes structured input to one renderer, keeping URL construction and HTML
escaping separate from delivery.

## How

Sender, brand, support address, subjects and verification/reset URL bases come from
[email config](../src/config/email.md). The renderer appends the bearer token as a URL query value
and escapes interpolated HTML. Resend receives both HTML and plain text.

`AUTH_EMAIL_VERIFY_TEMPLATE_PATH` and `AUTH_EMAIL_RESET_TEMPLATE_PATH` are parsed into configuration
but not loaded or applied. There is no template-file validation or expression engine in the current
renderer. Custom bodies require editing the renderer; configuring a path alone does not override it.
