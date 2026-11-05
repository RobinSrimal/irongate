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
GetItem and both Query paths request strong base-table consistency, including every continuation
page collected by `query_prefix`. This costs more read capacity than eventual reads, does not make
query pages a snapshot, and does not make a later write atomic with a read. Transactional Put replaces
the whole JSON `value` and TTL: Some sets the supplied deadline, while None removes any old expiry.
Every replacement chooses its expiry explicitly and preserves its previous-value condition.

Non-expired physical items fail closed on malformed encoding: GetItem requires a string JSON
`value`, and both Query paths additionally require string `pk`/`sk` attributes. Missing/non-string
attributes or invalid JSON produce fixed sanitized DynamoDB errors, not missing records or silently
omitted query rows. Stored content and JSON/SDK diagnostics are not returned. Existing TTL filtering
and cursor behavior are unchanged; this does not broaden TTL validation. Typed record validation
remains in the store. In particular, malformed email ownership indexes cannot disappear from a
query and let account deletion falsely report successful ownership cleanup.

Transaction cancellations with only recognized condition/conflict reasons map to ConditionFailed
or TransactionConflict, with conflicts taking precedence. Missing, unknown or mixed operational
reasons remain DynamoDB errors. Service, timeout, dispatch and decoding failures use fixed safe
error text and bounded diagnostics; transport failure does not establish rollback. No application
retry is added. See the [classifier](../src/storage/dynamo_errors.md).

Logical expiry is checked by typed operations independently of DynamoDB cleanup. The internal
[adapter contract](../src/storage/adapter.md) is public for test injection; runtime imports are
restricted by a static boundary check. Memory tests have known semantic differences. Focused
[DynamoDB Local tests](../tests/dynamodb_storage.md) exercise replacement value/TTL semantics,
cancellation rollback, stale code consumption, invalid transactions and persistent account writes.
Their [refresh cases](../tests/dynamodb_storage/refresh.md) inspect physical TTL and stale rotation
transactions; neither suite establishes complete backend equivalence or every lifecycle race.
