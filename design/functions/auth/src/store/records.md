# Auth Records

Source: [packages/functions/auth/src/store/records.rs](../../../../../packages/functions/auth/src/store/records.rs).

## What

Serializable account, identity, password, proof, session, code and refresh records.

## Why

Typed records preserve the information needed for expiry, replay and lifecycle checks.

## How

Short-lived records carry expires_at independently of physical expiry. Account/identity statuses encode lifecycle state; optional password/contact fields support tombstones. Refresh records retain family/client/subject/scope and replacement/revocation metadata. There is no credential-generation field.
