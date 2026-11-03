# SDK HTTP Stub

Source: [packages/functions/auth/tests/support/dynamo_stub.rs](../../../../../packages/functions/auth/tests/support/dynamo_stub.rs).

## What

A loopback Axum responder with queued replies, captured requests and an in-memory tracing writer.

## Why

Wire contracts need deterministic HTTP failures without imitating DynamoDB persistence semantics.

## How

Binds an ephemeral loopback port and records SDK target headers and JSON bodies. Responses can carry
JSON, malformed bodies or redirect headers. Unexpected requests receive an error; callers assert
request counts to detect retries. Dropping the stub aborts its server task. The log writer captures
bounded test diagnostics for sensitive-data rejection checks.
