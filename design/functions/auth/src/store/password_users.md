# Password Users

Source: [packages/functions/auth/src/store/password_users.rs](../../../../../packages/functions/auth/src/store/password_users.rs).

## What

Pending password records, verified account creation and tombstones.

## Why

Verification and lifecycle rules belong in typed persistence rather than route-controlled writes.

## How

Email-digest lookup stores an Argon2 PHC hash and normalized email. Verification conditionally writes
the user, generation-zero account, identity and subject indexes. Deleted records lose password/contact
data, and reuse creates a new subject. Password replacement is available only through the
[recovery transaction](recovery.md), not an independent hash-update operation.
