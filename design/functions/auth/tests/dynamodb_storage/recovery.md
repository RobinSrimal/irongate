# Local Recovery Transactions

Source: [packages/functions/auth/tests/dynamodb_storage/recovery.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/recovery.rs).

## What

Atomic recovery, rejection, TTL and uncertain-outcome contracts against disposable DynamoDB Local.

## Why

Recovery correctness includes actual stored state after success, genuine conditional rejection and
an error response, not only the returned status.

## How

Verified-account fixtures request multiple links, issue codes and rotate refreshes across clients.
Successful reset changes the password and generation once, removes used proof/index, rejects old
credentials and preserves unrelated subjects. Independent reads inspect logical expiry, physical
TTL and persistent account/user records. Bounded known-partition queries compare complete state
without printing hashes or digests.

Isolated mutations exercise purpose, expiry, subject/email binding, generation, verified/deleted
user, account status and malformed missing-generation rejection. Weak passwords, counter overflow
and expiry between preparation/finish leave recovery state unchanged. A changed user forces a real
failed transaction; fault gates distinguish no dispatch from a lost response after a real commit.
The latter returns an operational error while independent reads prove completed recovery.

Tests are ignored by default and run through the local target's safe unique-table fixture. They
do not establish production retry reliability, TTL cleanup scheduling or JWT revocation.
