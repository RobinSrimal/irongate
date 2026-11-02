# Email Rendering

Source: [packages/functions/auth/src/email/templates.rs](../../../../../packages/functions/auth/src/email/templates.rs).

## What

Built-in verification and reset HTML/text messages.

## Why

Message construction keeps bearer links and escaping consistent across password flows.

## How

The renderer appends a token query parameter to configured URL bases, escapes HTML values and applies configured brand, support address and subjects. Configured template-path fields are not read by this renderer.
