# Provider Issuance Outcomes

Source: [packages/functions/auth/tests/support/provider_issuance/outcomes.rs](../../../../../../packages/functions/auth/tests/support/provider_issuance/outcomes.rs).

## What

Shared success, independent-condition rejection, preparation rejection and deleted-reuse cases.

## Why

Provider issuance must preserve identity authority, existing credentials and exact snapshot
conditions while producing single-use codes bound to the consumed authorize session.

## How

New and returning Google cases inspect all committed records, consume usable codes, preserve
generation and subject across changed/missing email, and verify narrow code claims. A verified
password account created through the registration fixture shares the email but keeps a distinct
subject and unchanged credential, account, identity, email owner and reverse indexes. Neither a
matching nor changed provider email creates an ownership record/index for the provider subject;
an unclaimed changed email remains unclaimed. Apple's ownership contracts live in [apple.md](apple.md).

Each new/deleted write condition and each returning account/identity/code condition independently
rejects a gated real transaction. Exact candidate keys remain unchanged, including no orphan account
or index; code collisions preserve another identity's actual usable code. Malformed identity/account
bindings, missing/inactive accounts, missing generation, provider/session mismatch, unsupported
password issuance and invalid TTLs fail before issuance. Deliberately invalid fixtures are isolated
from ordinary seeding. Never/immediate/retention cases preserve tombstones on rejection and require
a fresh subject on allowed reuse, without reviving old accounts or indexes.
