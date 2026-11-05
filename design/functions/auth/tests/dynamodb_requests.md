# DynamoDB Wire Contracts

Source: [packages/functions/auth/tests/dynamodb_requests.rs](../../../../packages/functions/auth/tests/dynamodb_requests.rs).

## What

Normal Cargo integration tests using the real AWS SDK against an HTTP loopback stub.

## Why

Wire serialization and service error deserialization need coverage independent of the memory backend.

## How

Checks GetItem and both Query consistency flags, empty-page continuation, cancellation precedence,
operational service/transport/timeout/invalid-response errors, and refusal to follow redirects.
Sensitive synthetic messages, item contents and unknown codes must not reach adapter display text,
captured transaction logs, OAuth server-error bodies or generic storage responses. Typed code consumes
convert only a confirmed condition failure to None after reading a valid active generation-bound
account. Endpoint safety checks reject nonlocal targets.

A parameterized real-SDK query test supplies missing or numeric `pk`/`sk` attributes to both query
methods. Both must fail operationally rather than skipping a row or substituting an empty key;
the existing safe-error helper checks logs and HTTP error surfaces for synthetic secret markers.
These malformed response keys require a wire stub because real DynamoDB tables require key
attributes. Physical value corruption is instead tested against real Local tables in the
[ownership suite](dynamodb_storage/email_ownership.md).

The SDK has no retries and a bounded operation timeout. These tests do not prove database atomicity.
