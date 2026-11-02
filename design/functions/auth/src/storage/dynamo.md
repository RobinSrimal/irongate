# DynamoDB Adapter

Source: [packages/functions/auth/src/storage/dynamo.rs](../../../../../packages/functions/auth/src/storage/dynamo.rs).

## What

AWS SDK item encoding, exact-key operations, partition queries and transactions.

## Why

Auth record serialization and conditional expressions are centralized.

## How

Items contain pk/sk, JSON-string value and optional numeric expiry. Reads filter expired items but are not configured strongly consistent. Queries paginate a partition; query_prefix collects its results. Update changes value only. Transaction cancellations currently collapse to TransactionConflict rather than classifying individual cancellation reasons.
