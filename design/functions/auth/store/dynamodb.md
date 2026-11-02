# DynamoDB Storage

## What

The typed [AuthStore](../src/store/mod.md) sits above the
[DynamoDB adapter](../src/storage/dynamo.md). Route/provider code receives purpose-specific operations,
not raw item primitives.

## Why

One backend owns JSON serialization, condition expressions, exact-key calls and partition queries.

## How

Items contain `pk`, `sk`, JSON-string `value` and optional numeric `expiry`. One-time consumes attach
conditions to the same Delete item; no transaction may separately check and write the same item.
The current adapter uses default-consistency reads, collects all query pages in `query_prefix`,
and does not classify individual transaction cancellation reasons. Update replaces `value` but
leaves physical TTL untouched.

Logical expiry is checked by typed operations independently of DynamoDB cleanup. The internal
[adapter contract](../src/storage/adapter.md) is public for test injection; runtime imports are
restricted by a static boundary check. Memory tests have known semantic differences and are not
DynamoDB Local contract tests.
