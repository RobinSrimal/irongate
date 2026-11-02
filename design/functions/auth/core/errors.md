# Errors

Implementation: [error.rs](../src/error.md).

## What And Why

Shared OAuth, auth and storage error enums give route code consistent status/body handling and let
store callers distinguish conditional conflicts from ordinary failures where the backend exposes
that distinction.

## How

OAuth errors serialize protocol categories; auth errors cover request, account and rate-limit
failures; storage errors carry backend and transaction failures.
UserInfo additionally formats Bearer challenges at its endpoint.

Some server errors carry internal strings through the current error mapping. This file is not a
general redaction layer, and current DynamoDB transaction cancellation handling does not classify
every SDK cancellation reason. See [storage semantics](../store/dynamodb.md).
