# Transaction Errors

Source: [packages/functions/auth/src/storage/dynamo_errors.rs](../../../../../packages/functions/auth/src/storage/dynamo_errors.rs).

## What

A private mapping from SDK transaction failures to existing StorageError variants.

## Why

A losing conditional consume is not the same as a conflict or outage. Service messages and
cancellation item contents can contain auth data and must not reach callers or logs.

## How

A nonempty cancellation list containing only None, ConditionalCheckFailed and TransactionConflict
is classified with conflict taking precedence over condition failure. Missing, empty, all-None,
unknown or mixed operational reasons become DynamoDB errors. Service, dispatch, timeout and
response failures also remain operational; none implies a rollback or credential rejection.

Operational diagnostics use a closed set of category literals and fixed public error text. SDK
messages, arbitrary error codes, reason messages and returned items are never formatted. The helper
adds no retries and changes neither transaction construction nor the classification of other APIs.
