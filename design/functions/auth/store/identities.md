# Identity Storage

## What

Provider identity mappings and subject reverse indexes in [AuthStore](../src/store/mod.md) and
[records](../src/store/records.md).

## Why

A verified provider identity needs a stable internal subject and indexed lifecycle cleanup.

## How

Password identities use a normalized-email HMAC; Google/Apple use purpose-separated issuer/subject
HMACs. New identities create an account and index transactionally. Existing active identities keep
their subject and update properties/last-seen after an account check. Equal email across providers
does not merge accounts.

Deletion strips identity subject/properties and records reuse timing. Reuse checks the tombstone
and conditionally creates a new account/identity/index set. There is no separate identities.rs or
authoritative shared email-ownership index in the current implementation.
