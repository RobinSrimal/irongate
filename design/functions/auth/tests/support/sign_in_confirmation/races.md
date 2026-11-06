# Confirmation Races

Source: [packages/functions/auth/tests/support/sign_in_confirmation/races.rs](../../../../../../packages/functions/auth/tests/support/sign_in_confirmation/races.rs).

## What

Backend-shared ordered confirmation races and failure-outcome contracts.

## Why

Deferred proof cannot silently authorize a changed account or leave partial credentials after a
confirmed conditional failure. A response error must not be mistaken for rollback.

## How

Transaction gates delegate real conditions to the supplied backend. Cases independently change
all five creation guards and all seven completion items for fresh and existing owners, including
a real usable code collision. Exact row snapshots detect partial writes and loser orphans.

Actual reset, disable, delete, owner/index rebinding and competing identity creation occur either
before pending creation commits or before completion is prepared, proving persisted snapshots
cannot adopt newer state. Both winner orders cover same/different Google identities, Apple issuance
and pending password completion. An older pending password cannot install a credential after
provider ownership wins.

Fault injection distinguishes pre-dispatch failure from successful commit followed by a deliberately
lost response. The stored challenge/code outcomes are inspected; no retry resolver or compensation
is assumed. These controlled races are not production network or every lifecycle-history certification.
