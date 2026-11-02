# Identity Model

## What

Password, Google and Apple identities map to generated persisted subjects. The implementation is in
[AuthStore](../src/store/mod.md), [records](../src/store/records.md) and provider modules.

## Why

Provider issuer/subject is identity proof; an email claim is not a universal account key.

## How

Password identity creation follows verification. Google/Apple identities follow signed-token
validation and use issuer/subject HMAC lookup. Subsequent login resolves the stored subject rather
than deriving an ID from current claims. Password and provider logins sharing an email currently
create separate accounts; there is no user-facing link/merge operation.

Deleted-identity reuse creates a fresh subject under the configured policy. See
[identity storage](../store/identities.md) for the transaction and tombstone details.
