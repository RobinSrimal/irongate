# Password Route Tests

Source: [packages/functions/auth/tests/api_routes/password.rs](../../../../../packages/functions/auth/tests/api_routes/password.rs).

## What

Password route response shape, redirects and rate-limit rejection.

## Why

Registration/verification/reset must remain distinct from authenticated code issuance.

## How

Imports the parent api_routes fixtures and invokes the real router against TestStorage. Covers password registration, verification, forgot/reset and login plus secret-safe rate-limit keys. It is compiled as a child module, not a separate Cargo target.
