# Provider Issuance Races

Source: [packages/functions/auth/tests/support/provider_issuance/races.rs](../../../../../../packages/functions/auth/tests/support/provider_issuance/races.rs).

## What

Shared competing-callback, lifecycle/identity race, expiry and uncertain-outcome cases.

## Why

A stale callback cannot adopt a new credential generation, overwrite another callback's metadata,
revive a deleted subject or mistake a lost response for transaction rollback.

## How

Two gated callbacks prepare the same new, active or reusable-deleted identity with different
properties and code digests. Candidate email addresses are unique per race and normalized so the
fixture tests fresh claims rather than accidentally reusing an earlier winner's ownership.
Each commit order has one winner; exact planned account/index/code
reads prove unchanged winner state and no loser orphans. Both orders also cover issuance against
disable/delete, missing accounts, conditional generation replacement and identity update/deletion.
When lifecycle wins, issuance rejects without metadata/code writes. When issuance wins, later
account changes make the code unusable through existing code-consumption guards. Identity-only
changes deliberately do not imply an extra identity-history revocation check.

Controlled clocks exercise expiry at preparation, finish and the final pre-dispatch boundary;
storage time and TTL cleanup are not advanced. Expiry/empty-code/overflow fixtures include valid email
authority so each failure isolates its intended boundary. Empty code digests and timestamp overflow write
nothing. Operational read/pre-dispatch failures retain original state, while response loss after a
real successful new/active/deleted transaction leaves the complete state and usable code committed.
The gate never substitutes fake transaction conditions, and no application retries are introduced.
