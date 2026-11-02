# Password Users

Source: [packages/functions/auth/src/store/password_users.rs](../../../../../packages/functions/auth/src/store/password_users.rs).

## What

Pending password records, verified account creation, password replacement and tombstones.

## Why

Verification and lifecycle rules belong in typed persistence rather than route-controlled writes.

## How

Email-digest lookup stores an Argon2 PHC hash and normalized email. Verification conditionally writes the user, account, identity and subject indexes. Hash replacement and deletion use stored-record conditions; deleted records lose password/contact data, and reuse creates a new subject.
