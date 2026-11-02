# Subject Utility Module

Source: [packages/functions/auth/src/subject/mod.rs](../../../../../packages/functions/auth/src/subject/mod.rs).

## What

Export of the property-based subject schema utility.

## Why

Keeps that utility's namespace distinct from core persisted subjects.

## How

Re-exports schema definitions; account and OAuth paths use core::subjects::Subject instead.
