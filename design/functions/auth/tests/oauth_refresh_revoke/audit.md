# OAuth Audit Tests

Source: [packages/functions/auth/tests/oauth_refresh_revoke/audit.rs](../../../../../packages/functions/auth/tests/oauth_refresh_revoke/audit.rs).

## What

Router coverage of code/refresh audit events, privacy and confirmed mutation ordering.

## Why

Output must describe real transitions without exposing bearer credentials or changing responses.

## How

The password-domain fixture supplies a verified account and real login code. Tests assert enabled
and none modes, exchange/rotation/reuse state, digest-only output, code replay, idempotent revoke and
unknown/foreign-token silence. Foreign family state stays active. Sensitive body/header/cookie/IP,
password/email/token markers never enter captured audit output. Failed output preserves success
and replay rejection; a failing signer after committed rotation still leaves a rotation event.
Memory-backed state checks do not certify DynamoDB races or deployed delivery.
