# Ownership Cleanup Races

Source: [packages/functions/auth/tests/support/email_ownership/races.rs](../../../../../../packages/functions/auth/tests/support/email_ownership/races.rs).

## What

Shared ordered deletion/reuse races and stale reverse-index cleanup conditions.

## Why

An old deletion attempt cannot tombstone a newly registered owner or delete its index. Even cleanup
of an obsolete index must validate the owner snapshot and exact index value at commit.

## How

A gate pauses real ownership cleanup; another deletion completes, then fresh proof completion
creates a different subject. Releasing the old transaction causes a conditional rejection with all
new credential/owner/index rows unchanged. Retrying deletion for the old subject remains harmless.

Stale-index fixtures cover absent owners, tombstones and actual newly registered owners. Cleanup
removes only the stale index. Independent competing owner and index changes reject the transaction
without partial writes, including an owner created after an absence check. Fixture-only corruption
is explicitly restored before retry; a changed owner never becomes permission to rewrite it.
Both backends use the same bounded gate and actual conditional transactions, not scheduled sleeps.
