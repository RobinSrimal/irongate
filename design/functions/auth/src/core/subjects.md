# Persisted Subjects

Source: [packages/functions/auth/src/core/subjects.rs](../../../../../packages/functions/auth/src/core/subjects.rs).

## What

An opaque account-subject wrapper.

## Why

User identity remains stable without deriving an identifier from mutable email or provider claims.

## How

New subjects use a user_ prefix and a UUID. Persisted values are wrapped without regeneration and exposed through as_str for records and claims.
