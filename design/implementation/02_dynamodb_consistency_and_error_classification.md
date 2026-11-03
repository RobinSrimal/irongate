# 02_dynamodb_consistency_and_error_classification

## Status

Implemented and locally verified on 2026-11-02, following
[repository alignment](01_repository_design_alignment.md). No deployment was performed.

## Goal

Make the existing DynamoDB adapter request strongly consistent auth reads and distinguish a failed
condition from a transaction conflict or operational failure.

This establishes reliable storage responses for the current auth flows without changing their
records, protocol, hosting or lifecycle orchestration. It does not transfer the entire downstream
storage implementation.

## Why This Slice

The baseline review and focused source comparison found two related adapter gaps:

- `get`, `query_prefix` and `query_prefix_page` omit `ConsistentRead`. A lookup immediately following
  a successful write can therefore observe stale base-table state.
- `transact` maps every `TransactionCanceledException` to `TransactionConflict`, discarding its
  cancellation reasons. Typed one-time stores already handle `ConditionFailed` as a rejected consume,
  so deployed behavior can differ from tests that return the correct condition error directly.

Fixing these at the adapter boundary benefits existing account, code and session operations without
introducing another auth abstraction. Strong consistency increases read-capacity consumption relative
to eventual reads. It does not make a read followed by a write atomic, make multiple query pages a
snapshot, or revoke already-issued access JWTs.

## Reviewed Sources

Destination baseline: `a1cc80c` in Irongate.

Reference repository: `/Users/robinsrimal/Desktop/64x64`, read-only, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620`. Its HEAD matched that commit and its worktree was clean
when this slice was defined. The earlier `28158b8` commit groups several storage fixes; do not
cherry-pick it wholesale.

Relevant reference paths, relative to that repository:

- `packages/functions/auth/src/storage/adapter.rs`: consistency and failure contract.
- `packages/functions/auth/src/storage/dynamo.rs`: consistent SDK reads and transaction dispatch.
- `packages/functions/auth/src/storage/dynamo_errors.rs`: cancellation-reason classification.
- `packages/functions/auth/tests/dynamodb_requests.rs`: real SDK requests against a loopback stub.
- `packages/functions/auth/tests/support/dynamo.rs`: disposable local-only table fixture.
- `packages/functions/auth/tests/support/contract.rs`: condition failure and transaction rollback cases.
- `packages/functions/auth/Cargo.toml`: test HTTP-client dependency, not a runtime dependency upgrade.

The reference's current test runners also include recovery, linking and rate-limit work absent from
Irongate. Adapt only the cases needed here. Preserve Irongate's existing constructors, typed-store
boundary and pinned SDK versions; no product identifiers, credentials or deployment config transfer.

## Design To Honor

- [Overview](../overview.md).
- [Adapter mirror](../functions/auth/src/storage/adapter.md).
- [DynamoDB mirror](../functions/auth/src/storage/dynamo.md).
- [Storage module mirror](../functions/auth/src/storage/mod.md).
- [Error mirror](../functions/auth/src/error.md).
- [DynamoDB overview](../functions/auth/store/dynamodb.md).
- [Authorization-code storage](../functions/auth/store/authorization-codes.md).
- [Auth testing](../functions/auth/testing.md).
- [Threat model](../functions/auth/threat-model.md).
- [Runtime IAM](../infra/auth/iam.md).

Current design documents accurately describe the gaps. Update them to the implemented behavior in
this slice, not in advance merely because this plan exists.

## In Scope

### Consistent Base-Table Reads

- Set `consistent_read(true)` on GetItem and both Query entry points.
- Preserve that setting on every page fetched by `query_prefix`, including continuation requests.
- Preserve existing exact partition/sort-prefix constraints, cursor handling and expiry filtering.
- State the consistency contract in `StorageAdapter` documentation. Queries are not a cross-page
  snapshot, and a read does not lock the returned record.
- Do not introduce a configurable weaker read mode or change table/index definitions.

### Transaction Failure Meaning

Use the existing `StorageError` variants. Keep classification private to the DynamoDB backend;
do not require route/provider code to inspect AWS SDK error types.

| Observed result | Adapter result |
| --- | --- |
| Successful transaction | `Ok(())` |
| Known reason list containing `ConditionalCheckFailed` and otherwise only `None` | `ConditionFailed`, with a fixed safe message |
| Known reason list containing `TransactionConflict`, optionally also conditions/`None` | `TransactionConflict` |
| Empty, absent, all-`None`, unknown or mixed operational cancellation reasons | Operational `DynamoDB` error |
| Access denied, throttling, capacity, validation or other service failure | Operational `DynamoDB` error |
| Dispatch, timeout or response-decoding failure | Operational error; never a credential rejection or success |

Unknown/operational reasons take precedence over a condition elsewhere in the list. A service
payload containing a familiar word is not evidence of a condition failure; inspect typed SDK
metadata rather than display-string matching.

The transaction mapping must not propagate service messages, cancellation item contents, request
payloads or credentials through `StorageError` display text or logs. Retain only bounded error
categories/codes useful for diagnostics. This is scoped to the transaction path, not a repository-wide
error-response rewrite.

Do not add automatic application retries. A failed transport response can leave the commit outcome
unknown. Only a confirmed transaction cancellation supports assertions about no writes taking effect.

### Existing Caller Behavior

Review existing `ConditionFailed` and `TransactionConflict` branches in typed stores. Tests must
show that a genuine losing one-time consume reaches the existing rejection result, while an
operational failure remains an error. Do not turn every backend error into `None`, invalid credentials,
reuse detection or success just to make tests pass.

Keep current public response contracts. Any required caller correction must be directly caused by
the newly accurate classification; stop and rescope if this expands into lifecycle or recovery design.

### Local Verification Boundary

Use two different forms of evidence:

1. An actual AWS SDK client aimed at a loopback HTTP stub proves request serialization and SDK
   error deserialization. It is not an emulator proving DynamoDB atomicity or read consistency.
2. A disposable DynamoDB Local table proves real adapter condition handling and all-or-nothing
   behavior for the limited transactions exercised here. Do not simulate this only with TestStorage.

The local fixture must use explicit dummy credentials, a fixed test region, bounded timeouts and
an explicit HTTP loopback endpoint. Reject non-loopback hosts, URL credentials, unexpected paths,
queries and fragments before making requests. Do not consult AWS profiles, the default credential
chain, SST outputs or environment-supplied deployed table names. Disable redirects where applicable.

Create a unique test-owned table and delete only that table, including on assertion failure where
the process remains alive. Do not scan or mutate a deployed table. If local prerequisites are absent,
report this validation as blocked rather than silently skipping it and declaring the slice complete.

## Keep The Change Surface Small

Expected production changes:

```text
packages/functions/auth/src/storage/adapter.rs
packages/functions/auth/src/storage/dynamo.rs
packages/functions/auth/src/storage/dynamo_errors.rs   new private helper
packages/functions/auth/src/storage/mod.rs
```

Expected test/support changes:

```text
packages/functions/auth/tests/dynamodb_requests.rs     new SDK wire tests
packages/functions/auth/tests/dynamodb_storage.rs      new focused local tests
packages/functions/auth/tests/support/dynamo.rs        new local-only fixture
packages/functions/auth/tests/README.md
```

A small additional test helper is acceptable if it keeps responsibilities separate. Reuse existing
dependencies first; add a compatible dev-only SDK HTTP-client dependency only if the test client
requires it. Update the auth manifest/lockfile deliberately, without upgrading the production SDK.

Do not expand the already sizeable shared store module for this work. Keep every source file below
1,000 lines, including inline tests. No new npm command or infrastructure helper is required merely
to wrap a documented Cargo command.

## Out Of Scope

- Replacing `TransactOperation::Update` with full-record conditional puts.
- Refresh-family TTL renewal and changes to expiry-boundary or pagination semantics.
- Consolidating or correcting the complete memory backend contract.
- Atomic password recovery, credential generations, account linking or native provider login.
- Audit storage/emitter changes, JWT/JWKS changes and example-session hardening.
- Table schema, permissions, API routes, deployment configuration or cloud resources.
- Cloud deployments, Lambda Web Functions, UI/device testing and changes to 64x64 or Plaza.

These exclusions keep the slice focused. Tests and completion notes must not imply that the other
storage gaps or auth races have been resolved.

## Required Tests

### SDK Wire Tests

- GetItem and both Query variants send `ConsistentRead: true`.
- A multi-page query retains consistency and forwards the expected continuation key.
- An empty page with a continuation key does not prematurely stop the existing query loop.
- The cancellation matrix above covers condition-only, conflict-only, mixed condition/conflict,
  validation/capacity/throttling, unknown, absent and all-`None` reasons.
- Access denied and a transport/invalid-response failure remain operational failures.
- Synthetic sensitive service text and cancellation item data do not appear in returned error text
  or transaction diagnostics. Check a caller's OAuth server-error response as well as the generic
  storage response; the latter alone already hides its body and is insufficient evidence.
- Fixtures explicitly bound retry/timeout behavior so tests cannot hang or hide extra attempts.

### DynamoDB Local Tests

- A conditional replacement succeeds only against the expected current record.
- A confirmed failed condition in a multi-item transaction returns `ConditionFailed` and leaves all
  involved records unchanged; an unrelated put in that transaction must not appear.
- Two conditional deletes of the same stored authorization-code record produce one successful
  consume and one existing typed-store rejection, not two successes or a generic backend failure.
- Duplicate-item/invalid transactions remain operational errors rather than credential failures.
- Endpoint safety checks reject nonlocal targets before any request, and cleanup deletes only the
  fixture-created table.

Do not copy the reference's entire storage/recovery contract matrix into this slice. Existing
memory-backed tests remain useful regressions but are not evidence of DynamoDB equivalence.

## Design And Setup Updates

Mirror each changed/new source, test and manifest at its exact mapped path. In particular add
`design/functions/auth/src/storage/dynamo_errors.md`, the two test mirrors and
`design/functions/auth/tests/support/dynamo.md` when their implementations exist.

Update the storage overview, adapter mirror, testing guide and relevant error documentation to
describe current consistency, failure categories and local-test safety. Remove the now-obsolete
eventual-read/cancellation-collapse statements from affected topic docs and `design/overview.md`,
but retain warnings about non-atomic read/write workflows, separate query pages, physical TTL,
memory-backend differences and self-contained JWT expiry. Correct any misleading blanket claim that
all OAuth server-error messages are already sanitized.

Document the DynamoDB Local prerequisite and explicit test invocation in the auth test README and
its mirror. Do not repurpose the deployed smoke-test instructions to run destructive local tests.
No setup instructions should require new cloud credentials or a deployment for this change.

## Verification

Before implementation, rerun the relevant baseline and record failures. After implementation:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Run rustfmt checks on changed Rust files without unrelated formatting churn. Separate pre-existing
Clippy warnings from regressions. Run `npm run typecheck` or `npm run test:setup` only if their inputs
change; native/web example rebuilds are unrelated to this backend slice.

The new SDK wire tests run in normal Cargo tests. If the DynamoDB Local target is explicitly ignored
by default, run it separately with the prerequisite documented by the implementation:

```sh
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:8000 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
```

An ordinary passing `cargo test` with ignored local cases is not completion evidence for those cases.
Do not start or install a container runtime implicitly; inspect available local prerequisites first.

## Acceptance

- All three read entry points request strong consistency, including query continuation pages.
- Known condition failures, known conflicts and operational/uncertain failures remain distinct.
- Transaction errors and diagnostics do not expose service payloads.
- Existing one-time consume behavior works through the real DynamoDB adapter.
- SDK wire tests and the focused disposable-local transaction tests have actually passed here.
- Existing auth tests and the admin build remain compatible; no production schema/API change occurs.
- Exact design mirrors and relevant topic/operator instructions match the implemented behavior.
- Remaining storage/refresh/recovery limitations are explicit; no unrelated transfer is bundled.

## Result

- All three DynamoDB read entry points request strong consistency, including continuation pages.
- The private transaction classifier preserves known condition/conflict distinctions and treats
  incomplete or operational failures as backend errors with fixed safe text and bounded log fields.
- Existing typed-store rejection branches required no changes. SDK tests and DynamoDB Local both
  exercise authorization-code conditional consumption through the real adapter.
- Added focused SDK wire tests, disposable local transaction tests and exact design mirrors.
  Updated affected storage/error/lifecycle overviews without removing remaining race/TTL warnings.
- Added only a dev HTTP-client dependency at the already resolved version. Production SDK pins,
  table schema, protocol responses, infrastructure and optional examples are unchanged.

## Verification Results

Run on 2026-11-02 in this repository; Rust commands used `--locked --offline` after the one-line
lockfile update for the dev dependency. The pre-change full auth baseline passed.

| Check | Result |
| --- | --- |
| Full auth `cargo test` | Pass; four DynamoDB Local cases ignored by default and run separately |
| `--test dynamodb_requests` | Pass: six tests, including cancellation matrix, safe display/log/OAuth responses, timeouts and redirects |
| `--test dynamodb_storage -- --ignored` | Pass: four cases on DynamoDB Local 3.3.2, Java 17, temporary in-memory database |
| Auth and admin `cargo check` | Pass |
| Auth `cargo clippy --all-targets` | Pass with existing warnings in unchanged code; none in new files |
| `npm run test:infra` | Pass |
| Changed Rust files, rustfmt with `skip_children=true` | Pass |
| Exact changed-file mirrors and source lengths | Pass: ten hand-maintained files mirrored; all below 1,000 lines |
| Local Markdown links and `git diff --check` | Pass |

Restricted sandbox attempts could not bind/connect to local listeners; the local-network tests were
rerun with permission and passed. The initial duplicate-item test also carried conditions and reached
a different rejection path; the final test isolates an unconditional invalid duplicate-item request.
The local fixture verified cleanup after success and panic without deleting another fixture's table.
The temporary emulator was stopped after validation. No live table, AWS profile or deployed service
was used. The reference repository remained unchanged.

TypeScript/setup/native/example checks were not run because their inputs did not change. Deployment,
production IAM, physical TTL cleanup and real-provider/device validation remain outside this slice.
Strong reads are not a cross-page snapshot or an atomic lifecycle barrier; value-only Update TTL,
memory-backend semantic differences and recovery/refresh work remain unchanged.
