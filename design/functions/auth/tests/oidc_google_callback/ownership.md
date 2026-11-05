# Google Callback Ownership

Source: [packages/functions/auth/tests/oidc_google_callback/ownership.rs](../../../../../packages/functions/auth/tests/oidc_google_callback/ownership.rs).

## What

Signed Google callback contracts for email authority and stable account selection.

## Why

Store tests cannot prove that a callback verifies the signature and derives authority from the
validated claims rather than request parameters or arbitrary metadata.

## How

Local RSA tokens pass through the real validator/router. Verified Gmail and signed hosted-domain
claims join actual password or Apple owners; code exchange returns a verified access JWT for that
subject without exporting hd. Owner rows remain unchanged and only one account exists.

Missing/unverified/malformed email, non-hosted third-party addresses, invalid hd and lookalike Gmail
domains reject with the same safe response whether an owner exists or not. Callback query overrides
cannot supply authority. A corrupted otherwise-valid token produces no issuance/ownership changes.
Returning identities keep their subjects when email/hd disappear or a later address belongs to a
different account, including unverified later claims. Tests use memory fixtures, not live Google.
