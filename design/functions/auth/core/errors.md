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

OAuth server errors serialize the caller's description; there is no general redaction layer.
The DynamoDB transaction path uses fixed safe messages and bounded log categories. Known condition
failures and conflicts remain distinct, while unknown or operational cancellation reasons, service
errors and transport failures stay operational errors. Other backend paths can still carry internal
strings. See [storage semantics](../store/dynamodb.md).
