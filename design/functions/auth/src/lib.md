# Auth Library

Source: [packages/functions/auth/src/lib.rs](../../../../packages/functions/auth/src/lib.rs).

## What

Public modules consumed by integration tests and the admin crate.

## Why

The two Lambda entrypoints share auth/storage types without sharing their HTTP route surfaces.

## How

Exports API, runtime config, core, crypto, email, protocol, provider and store modules. DynamoStorage is re-exported; StorageAdapter is public for integration fixtures, with runtime access restricted by the boundary validator.
