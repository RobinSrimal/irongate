# 03_explicit_replacements_and_refresh_expiry

## Status

Implemented and locally verified on 2026-11-03. Commit `9bc3e07` recorded the completed
[DynamoDB consistency and error classification](02_dynamodb_consistency_and_error_classification.md)
slice and this definition before implementation began. No cloud deployment was performed.

## Goal

Make every transactional record replacement specify both its complete value and its physical TTL,
and fix refresh rotation so the family's DynamoDB expiry matches its renewed logical deadline.

Use the existing conditional Put operation instead of maintaining a second, ambiguous Update
operation. Preserve the current OAuth contracts, record shapes, account lifecycle behavior and
deployment boundaries.

## Why This Slice

The current adapter's Update replaces the entire JSON `value` attribute but leaves `expiry`
unchanged. Its name also hides a test-backend difference: both memory fixtures merge JSON fields
for Update instead of replacing the complete value.

Refresh rotation already advances the family's logical `expires_at`. Its unchanged physical TTL
can nevertheless make the adapter hide that family, or allow DynamoDB cleanup to delete it, before
the renewed token expires. This can terminate a valid refreshed session prematurely.

There are seven production Update call sites:

- Rotation replaces the old refresh token and its family.
- Revocation replaces the family and, when present, its current token.
- Account disable, enable and deletion each replace the account status record.

Conditional Put already supports full-value comparison and explicit expiry. Reusing it fixes the
refresh bug and removes the semantic ambiguity without adding a new backend API or a TTL-only patch.

## Reviewed Sources

Destination: definition reviewed against Irongate `a1cc80c8525754aeb70a8d7f7aa68f53a8dd1474` plus
the completed slice 02 working-tree changes. Implementation started from commit `9bc3e07` after
those changes and the definition were committed.

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620`. Its HEAD matched this commit and its worktree was clean
when this slice was defined. Relevant paths relative to that repository:

- `packages/functions/auth/src/storage/adapter.rs`: transaction variants and explicit Put expiry.
- `packages/functions/auth/src/storage/dynamo.rs`: full-record replacement behavior.
- `packages/functions/auth/src/store/refresh.rs`: old-token, family and revoked-record expiry choices.
- `packages/functions/auth/src/store/mod.rs`: persistent account status replacements.
- `packages/functions/auth/tests/support/refresh_contract.rs`: renewal, rollback and competing writers.

Commit `28158b8` contains the earlier storage correction, but also broader fixture, clock, pagination
and IAM work. Do not copy that commit wholesale. The pinned refresh implementation also includes
credential-generation guards absent from Irongate; those are not part of this slice.

## Design To Honor

- [Overview](../overview.md).
- [Backend contract](../functions/auth/src/storage/adapter.md).
- [DynamoDB adapter](../functions/auth/src/storage/dynamo.md).
- [Transaction errors](../functions/auth/src/storage/dynamo_errors.md).
- [Typed store](../functions/auth/src/store/mod.md).
- [Refresh implementation](../functions/auth/src/store/refresh.md).
- [DynamoDB semantics](../functions/auth/store/dynamodb.md).
- [Refresh storage](../functions/auth/store/refresh-tokens.md).
- [Account storage](../functions/auth/store/accounts.md).
- [Account lifecycle](../functions/auth/core/account-lifecycle.md).
- [Tokens](../functions/auth/core/tokens.md).
- [TTL configuration](../functions/auth/config/ttls.md).
- [Testing](../functions/auth/testing.md).
- [Local test instructions](../functions/auth/tests/README.md).
- [Threat model](../functions/auth/threat-model.md).

Update authoritative documentation when implementation changes, not merely because this slice has
been defined. Keep current what/why/how descriptions and retain limitations this work does not fix.

## In Scope

### Explicit Full-Record Writes

- Convert all seven production Update uses to Put with the complete replacement value.
- Preserve comparison with the complete previously read `value` on the same Put item. Do not split
  a condition check and write for the same item into separate transaction entries.
- Require an explicit expiry choice at each caller: `Some(deadline)` writes/replaces TTL;
  `None` omits TTL from the replacement, removing any old expiry attribute.
- Remove `TransactOperation::Update`, its DynamoDB builder/helper and both memory backend branches.
  Do not retain a compatibility alias or introduce another patch/update abstraction.
- Keep Put, Delete and ConditionCheck, and preserve slice 02's strong reads and safe failure
  classification. A rejected condition is not a successful write; an operational failure is not
  evidence of replay or rollback. Do not add retries or change route error mappings.

This removes the Update-specific fake/real mismatch. It does not fix every memory-backend condition,
expiry or pagination difference, nor authorize a full fixture consolidation.

### Refresh Expiry Rules

Keep initial issuance and the existing sliding refresh lifetime policy. Rotation still uses the
accepted `new_expires_at` supplied by its caller; do not add a maximum session age, a new TTL setting
or a different policy for shortened TTL configuration.

| Record written | Logical expiry | Physical `expiry` |
| --- | --- | --- |
| Initial token, family and subject/client indexes | Existing issuance deadline | Same deadline, Unix seconds |
| Replaced old token | Preserve its original `expires_at` | Original deadline, Unix seconds |
| Rotated family | Accepted `new_expires_at` | New deadline, Unix seconds |
| New token and its subject/client indexes | Accepted `new_expires_at` | New deadline, Unix seconds |
| Revoked family | Preserve its current `expires_at` | That same current deadline, Unix seconds |
| Revoked current token, if present | Preserve its own `expires_at` | That token's deadline, Unix seconds |

Old index records keep their existing deadlines. Do not extend spent tokens or every historical
index to the latest family lifetime. Preserve replacement markers and replay detection for retained
records; no new promise of detecting replay after a record has expired or been removed.

The old-token replacement, family replacement, successor and successor indexes remain in one
transaction. Revocation must not remove TTL, shorten it accidentally or resurrect a changed family.
Keep the current client binding, runtime expiry checks, active-account check and idempotent logout
behavior. Account checks and JWT signing remain separate from rotation; this slice does not make
the entire issuance workflow atomic.

### Account Replacements

Convert disable, enable and account tombstoning to full-record conditional Put with `expiry: None`.
Accounts remain persistent. Preserve the previous-record comparison, status rules, timestamps and
terminal deleted state. Do not change subject IDs, deletion contents, indexed cleanup, admin IAM,
session invalidation policy or multi-step lifecycle orchestration.

### Focused Test Changes

Reuse the SDK and disposable DynamoDB Local fixtures from slice 02. Inspect raw `value` and numeric
`expiry` through the fixture-owned SDK client where necessary; do not expose raw storage through
production routes or add scans.

Replace the refresh test that only counts transaction/CAS calls with observable all-or-nothing
state evidence. Remove its CountingStorage wrapper if no longer needed; retain useful protocol,
client-binding, replay and logout tests.

Small test-only raw-item or transaction-gate helpers are acceptable. A race gate must delegate the
actual transaction to the real backend, not synthesize a successful write or condition rejection.
Do not import the reference's entire contract framework or add a production clock abstraction just
to test this TTL correction. Use explicit well-separated timestamps and raw TTL assertions instead
of sleeps or waiting for asynchronous DynamoDB TTL deletion.

## Change Surface

Expected production and fixture edits, relative to `packages/functions/auth/`:

```text
src/storage/adapter.rs
src/storage/dynamo.rs
src/storage/test_support.rs
src/store/mod.rs
src/store/refresh.rs
tests/support/mod.rs
tests/oauth_refresh_revoke.rs
tests/dynamodb_storage.rs
tests/README.md
```

Extend `tests/dynamodb_requests.rs` only for useful wire contracts not already established. Add a
small test module/helper if needed to keep responsibilities clear and every source file below
1,000 lines. `tests/oauth_refresh_revoke.rs` is currently 787 lines and `src/store/mod.rs` is 729;
do not grow them into catch-all modules. Add a mirror for every changed or new hand-maintained file.

No new runtime dependency, manifest change, table schema, public route, deployment configuration
or cloud resource is expected.

## Required Verification Cases

### Replacement Contract

- A conditional Put replaces the entire JSON value: a field omitted from the replacement is gone,
  not retained by a merge. Exercise both memory fixtures and DynamoDB Local at the touched boundary.
- A replacement with `Some(new_expiry)` writes that TTL; a replacement with `None` removes an old
  TTL attribute. Use raw local GetItem results, not only adapter-filtered values, for physical TTL.
- A stale expected value cannot replace a newer record or change its TTL. Existing slice 02 tests
  may supply this coverage when they actually assert the required state; avoid duplicating them.

### Refresh Storage

- Initial issuance creates the primary token, family and both indexes with matching deadlines.
- With an initial deadline T0 and a later rotation deadline T1, inspect actual stored items:
  family/new token/new indexes have T1 in both logical and physical fields; old token/old indexes
  retain T0. The family points at the successor and the old token has the replacement marker.
- The successor can rotate again and be revoked through the subject index. Revocation preserves
  the family/current-token deadlines; subsequent rotation is rejected and repeated logout remains
  idempotent. Wrong-client revocation must still leave the family usable.
- A confirmed cancellation of a refresh transaction leaves no partial successor/index records and
  no partial expiry change. Force a real failing condition or a stale writer at the storage boundary;
  a mock that merely returns ConditionFailed is not atomicity evidence.
- Two rotations that read the same original token/family must not both commit. Gate commits in a
  deterministic order: one succeeds, the stale writer fails its condition, winner state is preserved
  and no loser successor/index records remain. Later reuse retains the existing revocation behavior.
- A rotation paused before commit cannot overwrite a committed family revocation. Release it after
  revocation and verify a conditional failure, retained revocation/TTL and no orphan successor.
- Retain rejection coverage for logically expired tokens/families independently of physical cleanup.
  Do not describe raw TTL inspection as proof of the AWS background TTL deletion schedule.

Use real DynamoDB Local for TTL attributes, failed transaction effects and stale refresh writes.
Keep bounded exact-key reads or known-partition queries for assertions. Never inspect a deployed
table or print raw tokens/digests in test reports.

### Account And Existing Contracts

- Disable, enable and delete still enforce their existing status transitions and comparisons.
  Check a local account record remains without TTL after the converted lifecycle writes.
- Deleted accounts cannot be enabled. Existing admin/auth regression suites still pass.
- Slice 02 consistency, cancellation/error sanitization and local-fixture safety tests still pass.
- No remaining code path or test backend supports `TransactOperation::Update`.

## Design Updates During Implementation

Mirror the actual edits at their exact mapped paths, including `src/` and `tests/`. Update these
affected overviews rather than preserving obsolete warnings:

- `design/functions/auth/store/dynamodb.md`: complete replacement and explicit TTL semantics.
- `design/functions/auth/store/refresh-tokens.md`: renewed family TTL, retained old-token deadlines
  and revocation retention.
- `design/functions/auth/config/ttls.md`: physical/logical refresh expiry agreement; policy unchanged.
- `design/functions/auth/testing.md`: coverage added and remaining memory/DynamoDB differences.
- `design/functions/auth/threat-model.md`: remove the corrected value-only Update TTL limitation;
  retain other race, recovery, audit and token-lifetime limits.

The two memory fixture mirrors should no longer describe JSON-merging Update semantics, but must
retain their physical-versus-logical condition limitations. Keep the lifecycle mirror explicit about
non-atomic multi-step cleanup. Review operator/setup docs for consistency; no new credentials,
configuration or deployment is required, so do not add unnecessary setup steps.

## Out Of Scope

- Inclusive expiry-boundary changes, CAS semantics, pagination changes or full fake-backend parity.
- Shared test-backend consolidation or a repository-wide test cleanup.
- Credential generations, atomic recovery, account linking, provider/native changes or audit work.
- New session-lifetime policies, retention migrations, bulk backfills or compatibility layers.
- Redesigning refresh replay handling, adding retries, or coupling signing/lifecycle into transactions.
- IAM policy cleanup, infrastructure changes, deployment, live-provider or UI testing.
- Changes to 64x64, Plaza or either optional example.

Existing already-expired family records are not recovered by this change. Do not rewrite deployed
data or claim that changing code retroactively repairs sessions lost to prior cleanup.

## Implementation Checks

Run a focused baseline before edits, including the refresh/admin tests and slice 02 local suite.
After implementation run:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Run changed-file rustfmt, exact mirror/link/line-limit checks, and the separately enabled local suite:

```sh
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
```

Use the [local fixture instructions](../functions/auth/tests/README.md): explicit dummy credentials,
loopback endpoint, bounded timeouts, generated tables and cleanup. Do not assume a temporary emulator
from the preceding slice is still running. Report missing prerequisites or sandbox restrictions
honestly; ordinary Cargo tests with ignored local cases are not evidence that the local cases passed.
Separate pre-existing Clippy warnings from regressions. Run setup/TypeScript/example checks only if
their inputs actually change.

## Acceptance

- Transactional replacements use complete conditional Put with explicit expiry; Update is removed.
- Refresh family TTL follows the accepted renewed deadline atomically with rotation.
- Old-token expiry, revocation TTL, persistent accounts, conditions and protocol behavior are preserved.
- Actual local DynamoDB tests establish physical TTL, stale-write rejection and no partial commits.
- Auth/admin validation and relevant existing security regressions pass.
- Exact design mirrors and affected topic docs reflect implemented behavior and remaining limits.
- No cloud resources or reference repositories are changed, and no additional slice is created.

## Implementation Result

- Removed Update from the adapter, DynamoDB implementation and both memory fixtures. All seven
  callers use complete conditional Put with explicit expiry; previous-value comparisons remain.
- Rotation renews the family's logical and physical deadline in the same transaction as its
  successor. Old-token expiry, revocation deadlines and persistent account records are preserved.
- Replaced the transaction-count-only refresh test with real storage evidence. The local refresh
  module covers issuance rollback, TTL renewal, subsequent rotation/revocation, competing rotations,
  rotation versus revocation and logical expiry independent of cleanup.
- Added small raw-item and pre-commit gate helpers. The gate delegates unchanged transactions to
  DynamoDB Local; assertions use exact-key reads and bounded known-partition queries, not scans.
- Updated exact design mirrors, affected storage/TTL/testing/threat-model overviews and the local
  test guide. Setup settings and operator steps are unchanged. All touched source files remain
  below 1,000 lines.

## Verification Results

Verification ran in Irongate, not in the reference repository. Cargo commands below used `--locked
--offline` with locally available dependencies.

| Check | Result |
| --- | --- |
| Pre-edit refresh/admin regression baseline | Passed: refresh/revoke, lifecycle and deletion suites |
| Pre-edit DynamoDB Local baseline | Passed: four existing cases |
| Full auth `cargo test` | Passed; ten local cases ignored by default and run separately below |
| Explicit `--test dynamodb_storage -- --ignored` | Passed: ten local cases, including real rollback and stale-writer tests |
| Auth `cargo check` | Passed |
| Admin `cargo check` | Passed |
| Auth `cargo clippy --all-targets` | Passed with warnings in existing code; no new test/helper warnings |
| `npm run test:infra` | Passed |
| Changed-file rustfmt, design mirror/link/line checks and `git diff --check` | Passed |

Local tests used DynamoDB Local 3.3.2 on loopback port 18023, explicit dummy credentials and
fixture-created disposable tables. The in-memory emulator was stopped after validation. Tests do
not establish AWS TTL cleanup timing or certify a live deployment.

TypeScript, setup and example/native builds were not run because their inputs did not change.
No AWS/Cloudflare resources, reference repositories or deployed data were modified. Account-check,
signing, recovery and multi-step lifecycle atomicity limitations remain outside this slice.
