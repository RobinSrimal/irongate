# 10_verified_email_ownership_for_password_accounts

## Status

Implemented and locally verified on 2026-11-05 following the user's implementation request.

Builds on `0c51330` (atomic provider identity and code issuance). This definition was the only
pending file at implementation start. No deployment, cloud reconfiguration, reference-repository
edit or deployed-data cleanup was performed. Design mirrors and relevant operator docs are updated.

## Goal

Record ownership of a normalized email only when password registration successfully completes:

```text
pending email registration -> no email ownership
valid email proof + chosen password -> atomic account, credential and email ownership
account deletion -> anonymized email tombstone and indexed cleanup
```

This is a bounded prerequisite for the handoff's shared-account resolution, not account linking
itself. Password, Google and Apple continue to create separate accounts in this slice. Equal email
claims from providers neither create nor consume email ownership records yet.

## Why This Slice

Slice 08 made password activation a six-operation transaction. Slice 09 made provider identity
resolution and code issuance atomic. Neither establishes provider-independent email ownership.
Password identity keys use their own HMAC family and cannot serve as a general email-owner index.

Add ownership to the existing registration transaction and its deletion lifecycle first. Combining
this with Google email-authority rules, Apple relay handling, provider confirmation and linking would
expand both the policy and API surface substantially. Creating an unused ownership helper alone
would not establish its transactional or lifecycle guarantees.

## Reviewed Reference

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620` from the
[handoff](../research/irongate_auth_improvements_handoff.md).

Relevant reference files:

- `packages/functions/auth/src/store/email_ownership.rs`: ownership/index writes, exact-record
  conditions, reuse policy and subject-indexed deletion.
- `packages/functions/auth/src/store/shared_identity.rs`: how ownership participates in broader
  account resolution; its linking and retry planner are not part of this slice.
- `packages/functions/auth/src/store/records.rs` and `keys.rs`: email record and key definitions.
  The pinned reference has no `store/registration.rs`; Irongate's existing registration module is
  the integration point, rather than a copied reference registration implementation.

At definition time reference HEAD is `ed881e143075eb5a0337a9bd6b687dfe0eb91060`, with unrelated
mobile/marketing sharing changes in its worktree. Leave them untouched and compare the pinned
commit. Recheck status and HEAD before implementation.

Adapt the storage principle, not the entire resolver. In particular, the reference retains a
subject on email tombstones. Irongate's deletion contract removes identity-to-subject associations;
the new email tombstone must also clear its subject and remove its reverse index.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Account lifecycle](../functions/auth/core/account-lifecycle.md),
  [accounts](../functions/auth/store/accounts.md) and
  [identity storage](../functions/auth/store/identities.md).
- [Registration](../functions/auth/src/store/registration.md),
  [typed store](../functions/auth/src/store/mod.md),
  [records](../functions/auth/src/store/records.md) and
  [keys](../functions/auth/src/store/keys.md).
- [Password users](../functions/auth/src/store/password_users.md) and
  [DynamoDB adapter](../functions/auth/src/storage/dynamo.md).
- [Admin API](../functions/admin/api.md),
  [email setup](../../docs/setup/06-resend-email.md) and
  [local storage tests](../../packages/functions/auth/tests/README.md).

## Scope

In scope:

- Typed email-owner and subject-index records in the existing AuthTable.
- Ownership preparation and conditional writes inside password registration completion.
- Indexed deletion, anonymized tombstones and the existing deleted-identity reuse policy.
- Focused registration/lifecycle rejection and race tests on memory and DynamoDB Local.
- Exact design mirrors and relevant current-behavior/operator documentation.

Out of scope:

- Provider email matching, account merging, password enrollment onto an existing provider account,
  provider email-confirmation routes or changes to existing provider issuance.
- New hosted pages, native flows, SDKs, example UI or hosting changes.
- Apple grant persistence/revocation, deletion receipts, queues or a new lifecycle orchestrator.
- New tables, secondary indexes, IAM permissions, secrets, configuration flags or dependencies.
- Migration, lazy backfill, compatibility wrappers, deployment or live-account testing.

The new key families use the existing table-scoped Get/Query/transaction permissions. Do not broaden
IAM or expose ownership records through public or admin responses.

## Records And Keys

Reuse the existing normalized-email `LookupFamily::Email` HMAC digest and normalization rules.
Do not strip plus aliases, rewrite Gmail dots, infer relay destinations or introduce a new secret.

| Typed Key | Partition Key | Sort Key |
| --- | --- | --- |
| `email_owner(email_digest)` | `email:owner` | Email HMAC digest |
| `email_by_subject(subject, email_digest)` | `email_by_subject:{subject}` | Email HMAC digest |

An active owner records:

```json
{
  "email_digest": "<HMAC digest>",
  "subject": "user_<generated>",
  "deleted_at": null,
  "reusable_after": null
}
```

A deleted owner records:

```json
{
  "email_digest": "<HMAC digest>",
  "subject": null,
  "deleted_at": "<deletion timestamp>",
  "reusable_after": "<reuse timestamp or null>"
}
```

Use a separate minimal typed reverse-index record containing email digest and subject. Validate
key/value bindings and state combinations on reads: a missing subject is not an active owner, and
a deleted owner cannot retain a subject. Malformed records fail closed rather than becoming an
available address. Operational/deserialization failures remain distinguishable from policy denial.

These records contain no raw email, proof, password hash, bearer value or provider profile. Active
owners, reverse indexes and tombstones have no DynamoDB TTL. Retention governs claim eligibility;
it is not permission to erase a tombstone or rely on asynchronous TTL cleanup.

## Registration Contract

Keep the existing public registration/verification API, proof binding, password policy and safe
responses. Registration start and resend create neither ownership nor an account. Ownership can
only be written after the bound email proof and pending registration pass the existing checks.

Extend the private prepared registration snapshot with a validated ownership condition:

- Absent owner: require absence at commit.
- Eligible deleted owner: require the complete prepared tombstone value at commit.
- Active owner: reject completion without consuming the proof or replacing the pending user.
  Do not attach a password, merge accounts or generate an alternative owner. This includes an
  owner whose account is disabled, deleted or missing; it is not an invitation to repair data.
- Ineligible or malformed deleted owner: reject; do not bypass the identity reuse policy.

Both the password identity and email-owner tombstone must be eligible. Preserve the existing reuse
semantics: `never` rejects; `immediate` still requires stored reuse permission; `after_retention`
requires a reached `reusable_after`. Missing permission metadata is not immediate reuse. Use a
controlled clock for new eligibility/expiry cases and preserve the existing proof pre-dispatch check.

Preparation writes nothing. Finishing uses one freshly generated subject with generation zero for
the account, credential, identity and all indexes. Extend the existing transaction to eight operations:

| Operation | Condition |
| --- | --- |
| Delete verification proof | Exact prepared proof |
| Activate password user | Exact prepared pending user |
| Create account | Absent |
| Create password identity | Absent or exact eligible identity tombstone |
| Create identity subject index | Absent |
| Create password-user subject index | Absent |
| Create email owner | Absent or exact eligible email tombstone |
| Create email subject index | Absent |

No ownership write happens before or after this transaction. A confirmed condition failure leaves
all eight changes unapplied; it cannot consume the proof, install a password or leave an orphan
owner/account/index. Keep one application-level transaction attempt, with no unbounded retry or
re-resolution to an existing owner's subject. An uncertain response may follow an actual commit;
preserve that distinction and do not compensate by deleting ownership.

## Deletion And Reuse

Integrate ownership cleanup into `AuthStore::delete_account` before reporting successful completion.
The existing flow first marks the account deleted, then performs indexed cleanup. Preserve that
boundary: this slice does not make the complete deletion operation atomic.

Query only the requested subject's email-index partition, using the existing paginated query
mechanism. Validate index bindings and read each exact owner consistently. For each active owner
still pointing to that subject, one transaction must:

1. Replace the exact owner with a tombstone, clearing the subject and using the account's persisted
   deletion timestamp plus the existing reuse policy/retention configuration.
2. Delete the exact corresponding subject index.

If the owner is already tombstoned, absent or belongs to a different subject, cleanup must not
rewrite it. Removing a stale index must be conditionally safe. Malformed bindings and operational
failures are not successful cleanup. Repeated deletion must be safe after either a confirmed
transaction failure or a committed transaction whose response was lost.

Do not remove the index before its required ownership transition commits. Failure must remain
visible to the existing admin caller so another deletion attempt can finish cleanup. Do not return
a new receipt, leak owner data or imply whole-operation rollback. Disable and enable leave
ownership intact; password reset does not transfer it.

A reusable email and password identity may be claimed only by fresh proof completion with a fresh
subject. An old deletion retry must never tombstone a newer owner's record or delete its index.
Partial cleanup may conservatively block new registration until deletion is retried; do not turn
that condition into an automatic takeover or silently ignore it.

## Existing Data Boundary

No inference from password rows, provider claims or account metadata creates ownership implicitly.
Newly completed registrations use the single new contract. Existing login/recovery paths are not
changed to require an ownership lookup, but historical accounts are not certified as indexed by
this slice. Do not advertise complete cross-provider uniqueness or shared-account behavior.

Use fresh disposable data for acceptance. Document that an existing deployment needs a separately
approved data-transition decision before relying on complete ownership coverage; do not add a
backfill, clear tables or use deployed data during implementation.

## Expected Change Surface

Primary runtime files:

```text
packages/functions/auth/src/store/email_ownership.rs  (new, internal helpers)
packages/functions/auth/src/store/registration.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/storage/dynamo.rs        (malformed-row rejection)
```

Reuse the exact-value condition helper and existing typed storage boundary. Keep ownership planning
internal; do not expose generic transaction assembly to routes or add unused provider-linking APIs.

The adapter must reject malformed non-expired physical records rather than silently treating them
as missing query results or absent exact-key reads. Review found that skipped index rows could
falsely report successful deletion while retaining an email owner. Share strict key/value decoding
between both query paths and strict value decoding with GetItem, preserve existing TTL filtering,
and return sanitized operational errors. Verify this through raw DynamoDB Local item corruption.

Extend the existing registration contract runners, their shared fixtures and admin deletion tests.
Extract focused email-ownership/lifecycle contract modules only when needed to keep responsibilities
clear and every source/test file below 1,000 lines. Share meaningful cases between memory and Local
runners; do not duplicate whole suites or replace DynamoDB behavior with fake successful writes.

## Design And Operator Updates

For every changed source/test/README file, update its exact relative-path design mirror. Add
`design/functions/auth/src/store/email_ownership.md` and mirrors for any new test modules.

Update the relevant current-behavior sections in the overview, threat model, store/keys/identity
overviews, account lifecycle, registration and test instructions. Explain the new records, proof-only
claim, atomic boundary, tombstone privacy and partial-deletion retry semantics. Update email/setup
or operations guidance where it describes registration, deletion or existing-data coverage.

Keep provider docs truthful: matching email still does not link accounts. Do not describe planned
provider matching as implemented. Authoritative docs state current what, why and how; completion
evidence belongs in this slice, not decision-history paragraphs spread through the design tree.

## Verification

Extend observable contracts to cover:

- Start/resend/invalid or expired proof never claims email. Successful completion creates one
  matching owner/index alongside a usable verified password account, without issuing tokens.
- Independent condition failure at each activation operation rolls back the entire transaction.
  Competing completions produce at most one owner and no losing account/index/credential writes.
- Active owners, invalid bindings, ineligible tombstones and changed ownership snapshots reject
  without attaching a password to another subject or consuming a still-valid proof.
- Deletion removes owner subject linkage/index and preserves the digest/reuse timestamp without
  raw email. Repeat deletion is safe; cleanup failure is visible and a retry can finish it.
- Never/immediate/retention reuse, missing reuse permission and exact boundary time; new ownership
  always has a new subject. Stale completion and old deletion retries cannot overwrite a winner.
- A gate pauses actual storage work to order deletion/reuse/competing claims. Inspect persisted
  outcomes on both backends, including pre-dispatch failure versus response loss after real commit.
- DynamoDB Local physical rows have no TTL for owners/indexes/tombstones. Verification proof TTL
  and runtime expiry remain intact. Existing provider no-linking and admin sanitization tests pass.

Use the existing disposable loopback-only DynamoDB Local setup, dummy credentials, unique test
tables and deterministic gates/clocks. Never point these tests at a deployed table or scan one.

Start with the focused registration and deletion baseline, then run the affected contracts and
full auth regression suite. Commands selected from current manifests:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test registration_contract --test password_registration --test admin_deletion
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Check formatting of touched Rust files, source length and design mirrors/links. Record unrelated
baseline formatting/lint failures instead of fixing unrelated modules. Run additional setup or
example checks only if their contracts change; no UI, native build, live email/provider or cloud
test is required for this storage-only slice. Stop the disposable emulator after validation.

## Acceptance Criteria

- Pending registrations do not reserve email; proof completion atomically creates ownership.
- Every new owner/index is bound to the same fresh subject as the completed password account.
- Ownership and identity reuse checks cannot overwrite an active owner or revive an old subject.
- Account deletion clears ownership's subject linkage and supports safe indexed retry.
- Memory and real DynamoDB Local contracts cover rejection, races and uncertain outcomes.
- Existing provider separation, admin isolation and credential-generation guarantees remain intact.
- Design mirrors and relevant operator docs describe actual implemented behavior and data limits.
- Completion records actual verification results and blockers, without claiming deployment or
  completion of the broader shared-account handoff.

## Implementation Result

- Added typed email-owner and subject-index keys/records plus an internal ownership module.
  Readers validate key/value bindings and active/deleted shapes, reject unknown fields, retain raw
  values for conditional comparisons, and return sanitized malformed-record errors.
- Password completion now commits all eight operations together. Ownership preparation is
  read-only; active owners reject without account repair or linking. Pending requests/resends do
  not claim an address. Both identity and owner reuse policies must permit completion.
- Existing account deletion performs ownership cleanup after credential/session cleanup and before
  success. Each owner transition and exact index deletion commits together. Stale indexes use a
  conditional owner absence/equality check and cannot rewrite another subject's ownership. Errors
  stay visible and the existing delete action can retry after partial cleanup or response loss.
- Email tombstones clear the subject. Owner/index/tombstone rows have no TTL or raw email. Existing
  login/recovery and provider account separation remain unchanged; no historical backfill was added.
- Review exposed an existing adapter issue: malformed physical rows were silently absent from
  Query results or GetItem reads. Both query paths now require string keys and a valid JSON-string
  value; GetItem uses the same strict value decoder. Existing expiry filtering is unchanged. Raw
  Local corruption tests prove failed cleanup remains visible and can complete after fixture repair.
- Tests extend the existing registration, admin deletion and provider contracts. Shared ownership
  contracts live in `tests/support/email_ownership.rs` and its lifecycle/race children, and run through
  memory and DynamoDB Local. The wire-stub suite covers malformed query keys that DynamoDB itself
  cannot persist. All touched test modules have exact design mirrors.

## Verification Results

| Check | Result |
| --- | --- |
| Pre-change registration/password/admin deletion baseline | Passed: 23 tests |
| Final full auth test suite | Passed: 311 test executions; 58 Local tests ignored in this command |
| Final full DynamoDB Local suite | Passed: all 58 ignored tests explicitly run, including 11 ownership cases |
| Auth `cargo check --locked` | Passed |
| Admin `cargo check --locked` | Passed after the adapter change |
| Auth Clippy, all targets | Passed with existing warnings; no unrelated lint cleanup |
| `npm run test:infra` | Passed |
| Touched Rust formatting and source-size check | Passed: 17 files, each below 1,000 lines |
| Exact mirrors and changed Markdown links | Passed: 18 source/README mirrors and 170 local link targets |
| `git diff --check` and stale six-operation documentation scan | Passed |
| Repository-wide `cargo fmt -- --check` | Still fails on pre-existing untouched discovery/config/crypto formatting |

The Local suite used an in-memory emulator bound only to `127.0.0.1:18023`, dummy credentials and
fixture-owned disposable tables. Local socket permission was required. The emulator was stopped
after verification and the port was confirmed clear. No deployed table or real account was used.

Final local evidence logs:

```text
/private/tmp/irongate-slice10-auth-final.log
/private/tmp/irongate-slice10-dynamodb-final.log
/private/tmp/irongate-slice10-clippy-final.log
/private/tmp/irongate-slice10-fmt.log
```

No TypeScript/example/native rebuild, setup-script rerun, UI test, live email/provider test or cloud
validation was performed: their source/configuration contracts are unchanged. Local tests establish
the exercised storage behavior, not complete production fault coverage, automatic repair of
historical/corrupt data, cross-provider uniqueness or globally atomic account deletion. The broader
shared-account handoff remains unfinished; no next slice is defined here.
