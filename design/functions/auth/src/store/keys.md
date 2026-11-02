# Storage Keys

Source: [packages/functions/auth/src/store/keys.rs](../../../../../packages/functions/auth/src/store/keys.rs).

## What

Central constructors for partition/sort keys and reverse-index partitions.

## Why

Consistent key families allow exact lookups and subject-scoped cleanup.

## How

Constructors accept already-computed digests for bearer values and identity lookups. Families include account, identity, password:user, oauth:session/code/refresh and refresh-family indexes. Inline tests check secret inputs do not appear in constructed keys.
