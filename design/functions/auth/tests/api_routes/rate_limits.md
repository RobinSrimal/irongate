# Independent Rate-Limit Route Tests

Source: [packages/functions/auth/tests/api_routes/rate_limits.rs](../../../../../packages/functions/auth/tests/api_routes/rate_limits.rs).

## What

Email/source rotation and client-authentication admission regressions through the real router.

## Why

Repeated identical requests cannot detect a limiter bypass caused by changing one identity component.

## How

Register/login/forgot vary trusted IP at a fixed normalized email, then vary emails at a fixed IP.
Registration sends only email; verify/reset send token and new_password. These current payloads
reach the limiter rather than failing DTO validation. Known accounts are completed through
email-first registration before the test. Known and unknown accounts exercise the same email policy. Denials leave protected-read/mail counts
unchanged, source denial creates no extra email counter, and unrelated pairs still work. Verify/reset
vary proofs without resetting source quota; the first admitted request must reach protected
storage, and denial must add no protected reads. OAuth tests separate a configured sentinel-like name
from unknown clients, count failed Basic/Post authentication before bearer reads and ensure authorize
uses query client_id rather than a Basic header. Uses shared admission helpers and memory storage;
real counter atomicity is covered by the Local contract target.
