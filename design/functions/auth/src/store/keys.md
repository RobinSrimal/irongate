# Storage Keys

Source: [packages/functions/auth/src/store/keys.rs](../../../../../packages/functions/auth/src/store/keys.rs).

## What

Central constructors for partition/sort keys and reverse-index partitions.

## Why

Consistent key families allow exact lookups and subject-scoped cleanup.

## How

Constructors accept computed digests for bearer values and identity lookups. Families include
account, identity, password:user, oauth:session/code/refresh and subject-scoped reverse indexes.
Inline tests check secret inputs do not appear in constructed keys.

`email_owner(email_digest)` uses partition `email:owner` and the normalized-email HMAC digest as
sort key. `email_by_subject(subject, email_digest)` uses `email_by_subject:{subject}` with that same
sort key; `email_by_subject_pk` selects only one subject's ownership indexes for cleanup. These use
the existing `LookupFamily::Email` digest, not a new secret or provider-identity digest.
No raw email enters either key. These are ordinary AuthTable records, not DynamoDB secondary indexes.
