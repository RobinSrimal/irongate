# Local DynamoDB Fixture

Source: [packages/functions/auth/tests/support/dynamo.rs](../../../../../packages/functions/auth/tests/support/dynamo.rs).

## What

A loopback-only SDK client and unique-table lifecycle for local integration tests.

## Why

Destructive contract checks must never discover a deployed table or borrow deployment credentials.

## How

Accepts only plain HTTP loopback URLs without credentials, non-root paths, queries or fragments.
Localhost is pinned to a literal address; the HTTP connector does not follow redirects. Clients use
dummy credentials, a fixed region, disabled retries and a two-second operation timeout, never the
default credential chain.

The local runner reads only DYNAMODB_LOCAL_ENDPOINT, generates an irongate-contract UUID table with
pk/sk, and awaits the test in a task. Cleanup deletes its privately retained table name before
resuming a test panic, even if the closure panics before returning a future. Termination of the
process or emulator can prevent cleanup; an in-memory disposable instance bounds that risk.

LocalTable also supports strongly consistent exact-key raw item reads. RawItem separates the parsed
JSON value from numeric physical expiry so tests can verify TTL independently of adapter filtering.
These inspection helpers exist only in test support and never expose raw records through auth APIs.
