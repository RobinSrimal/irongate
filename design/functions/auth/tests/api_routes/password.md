# Password Route Tests

Source: [packages/functions/auth/tests/api_routes/password.rs](../../../../../packages/functions/auth/tests/api_routes/password.rs).

## What

Password route response shape, redirects and rate-limit rejection.

## Why

Registration/verification/reset must remain distinct from authenticated code issuance.

## How

Imports the parent api_routes fixtures and invokes the real router against TestStorage. Covers
password registration, verification, forgot/reset and login plus secret-safe rate-limit keys.
Tests configure source/secondary policies explicitly; privacy assertions query bounded endpoint
partitions, require nonempty rows and
reject raw IPs and flow secrets. Rotating identities and early rejection are covered in sibling
[rate_limits](rate_limits.md) and [admission](admission.md) modules. This is not a separate Cargo target.
