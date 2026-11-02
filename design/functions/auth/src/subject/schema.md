# Property-Based Subject Utility

Source: [packages/functions/auth/src/subject/schema.rs](../../../../../packages/functions/auth/src/subject/schema.rs).

## What

A separate Subject value and shallow schema validator.

## Why

This library utility represents typed property bags, not persisted auth account identity.

## How

Its id hashes subject type and serialized properties with SHA-256. Validation checks type and required property presence. Live auth uses core/subjects.rs generated IDs; this utility must not be confused with that storage contract.
