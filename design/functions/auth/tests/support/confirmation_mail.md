# Captured Confirmation Mail

Source: [packages/functions/auth/tests/support/confirmation_mail.rs](../../../../../packages/functions/auth/tests/support/confirmation_mail.rs).

## What

Shared in-process email sender for confirmation delivery and completion tests.

## Why

Completion must use the actually delivered code without live email or duplicate sender fixtures.

## How

Captures recipient and rendered email privately, extracts the single eight-ASCII-digit text line
without numeric conversion and exposes messages only to tests. Constructors simulate acceptance
followed by failure or a bounded Notify-based delay. No remote request or email logging occurs.
