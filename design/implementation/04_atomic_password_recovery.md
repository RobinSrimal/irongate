# 04_atomic_password_recovery

## Status

Implemented and locally verified on 2026-11-03. No deployment or populated-stage data change was
performed. This slice follows the committed
[explicit replacements and refresh expiry](03_explicit_replacements_and_refresh_expiry.md) slice.

## Goal

Make password recovery one conditional storage commit: consume the reset proof, replace the
password hash and invalidate credentials issued under the account's previous credential generation.

After a successful reset, older reset links, unexchanged authorization codes and refresh tokens for
that subject must fail. New-password login must work. Merely requesting a reset must not invalidate
anything, and a confirmed rejected transaction must not burn the link or change the password.

## Why This Slice

`complete_password_reset` currently consumes the reset record and its index before loading the
password user, checking account activity, hashing the replacement and writing the password. A
failure between those steps can consume a valid link without changing the password. Other reset
links, authorization codes and refresh sessions are not invalidated by that password change.

The storage fixes from slices 02 and 03 provide consistent reads, accurate conditional-failure
classification and explicit full-record replacements. Recovery can now use those operations
without treating a read followed by an unrelated write as an atomic security boundary.

This slice includes the minimum generation checks in credential issuance/use needed to make reset
invalidation real. It does not transfer the reference's wider account-linking or provider system.

## Reviewed Sources

Irongate baseline: `a83ab91f3b4438b3a7770dc3e9655906cef19058`, clean when this slice was defined.

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620`. HEAD matched and its worktree was clean. Relevant files
relative to that repository:

- `packages/functions/auth/src/store/recovery.rs`: prepared snapshots and conditional recovery commit.
- `packages/functions/auth/src/store/credential_generation.rs`: active generation checks and guards.
- `packages/functions/auth/src/store/records.rs`: credential bindings.
- `packages/functions/auth/src/store/authorization_codes.rs`: account and verified-password guards.
- `packages/functions/auth/src/store/refresh.rs`: generation-bound issuance and rotation.
- `packages/functions/auth/src/providers/password.rs`: prepare, hash, finish orchestration.
- `packages/functions/auth/src/oauth/token.rs`: propagation of the authenticated generation.
- `packages/functions/auth/tests/support/recovery_contract.rs` and `recovery_races.rs`: behavioral
  contracts, failure outcomes and competing writes.

Earlier commit `69a92d1` explains the recovery change. Inspect the pinned source rather than
cherry-picking mixed commits. Do not import its optional-field compatibility policy, broad test
framework, shared-identity resolution or product deployment values.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Password domain](../functions/auth/core/passwords.md),
  [password API](../functions/auth/api/providers/password.md) and
  [password implementation](../functions/auth/src/providers/password.md).
- [Accounts](../functions/auth/store/accounts.md),
  [account lifecycle](../functions/auth/core/account-lifecycle.md),
  [password users](../functions/auth/store/password-users.md) and
  [password proofs](../functions/auth/store/password-secrets.md).
- [Authorization codes](../functions/auth/store/authorization-codes.md),
  [refresh records](../functions/auth/store/refresh-tokens.md),
  [tokens](../functions/auth/core/tokens.md) and
  [token endpoint](../functions/auth/src/oauth/token.md).
- [Store](../functions/auth/src/store/mod.md),
  [records](../functions/auth/src/store/records.md),
  [transaction contract](../functions/auth/src/storage/adapter.md) and
  [DynamoDB semantics](../functions/auth/store/dynamodb.md).
- [Testing](../functions/auth/testing.md),
  [local fixture instructions](../functions/auth/tests/README.md) and
  [Resend setup](../../docs/setup/06-resend-email.md).

Authoritative design describes current behavior. Update it with implementation, not in advance
because this slice has been defined. Preserve the API-only core and separate IAM admin Lambda.

## Scope Boundary

One security feature is in scope: atomic password reset with subject-wide invalidation of older
server-side credentials. Its necessary dependencies are an internal account generation, conditional
credential guards and existing caller/fixture updates.

No new service, table, index, route, secret, runtime dependency or infrastructure change is required.
Records gain fields inside the existing JSON `value`; physical keys and TTL configuration stay the
same. No AWS/Cloudflare deployment or data migration is authorized.

Keep the implementation focused:

- Put recovery orchestration in `store/recovery.rs`, not the shared store root.
- Put generation lookup/condition helpers in a small `store/credential_generation.rs` module.
- Change provider callbacks only to bind code issuance to an active account generation. Preserve
  provider validation, JWKS caching, identity lookup and callback semantics.
- Update existing tests where their contract changed; do not keep a second legacy reset path.
- Reuse the disposable-local fixture and transaction gate. Do not consolidate all memory storage
  backends or copy the reference's entire testing framework.

## Internal Credential Contract

Add a required `credential_generation: u64` to these records:

| Record | Rule |
| --- | --- |
| Account | Initialize to zero in every account-creation path; increment on successful recovery |
| Password reset | Capture the active account generation when the link is created |
| Authorization code | Bind to the generation authorized by login/provider completion |
| Refresh token | Carry the generation of the consumed code or rotated token |
| Refresh family | Match its token's subject, client and generation |

Use checked increment; exhaustion fails without writes. The counter is an internal persisted
authorization boundary, not a configurable option or a new JWT claim. Do not include it in sanitized
admin responses. Account lifecycle replacements retain the current counter and their existing
status/previous-value conditions. Account and password-user records remain without TTL.

Use a clean record contract. Do not silently accept missing generations as zero, stamp old
credentials with the latest generation, or add dual readers/backfills. Unversioned or malformed
records fail closed; no credential may be issued from them. This changes stored JSON compatibility.
Record that deployment to an existing populated stage requires a separately approved data decision;
do not reset a table or rewrite deployed records as part of implementation.

## Recovery Flow

### Request A Link

Keep the current normalized-email HMAC lookup, generic forgot-password response and Resend delivery.
Only a verified, non-deleted password user bound to an active account is eligible.

Create the reset proof and subject index in one transaction, with conditions on the complete
password-user and active-account snapshots used to decide eligibility. The proof records the account
generation and `purpose=reset_password`; proof/index logical expiry and physical TTL remain aligned.

Send email only after the creation transaction succeeds. A confirmed eligibility/condition race
returns the same generic response without sending a link. Operational or uncertain failures remain
errors, not evidence that the email address is unknown or that a link was successfully sent.

### Prepare, Hash, Commit

Introduce an opaque prepared-reset value whose snapshots cannot be edited by callers:

1. Look up the HMAC-keyed proof without consuming it. Validate purpose and logical expiry.
2. Load the bound password user and account. Require a verified, non-deleted user with a password
   hash, matching subject, active account and matching credential generation.
3. Validate password policy before writes and compute the new Argon2id hash. Preparation must not
   consume the proof, so hashing failure cannot burn it.
4. Recheck proof expiry immediately before submitting the final transaction, including time spent
   hashing. Use a narrowly scoped injectable recovery clock for deterministic boundary tests if
   needed; do not introduce a repository-wide clock framework or sleep-based tests.
5. Commit all four operations together:

| Operation | Condition / expiry |
| --- | --- |
| Delete used reset proof | Complete previously validated proof still matches |
| Delete its subject index | Exact index key; no partition-wide cleanup |
| Replace password user with new hash/timestamps | Complete validated user still matches; no TTL |
| Replace account with incremented generation | Complete validated active account still matches; no TTL |

Put the previous-value condition on the replacement itself. DynamoDB must not receive both a
ConditionCheck and Put for the same item. Preserve subject, account status and identity ownership;
recovery never reactivates, recreates or merges an account.

Return the existing password-reset success shape only after confirmed success. There is no
automatic login and no tokens in the response. Remove the obsolete independent
`consume_password_reset` and `update_password_hash` APIs after updating all callers/tests. Keep
email-verification consumption and indexed lifecycle cleanup; verification atomicity is not part
of this slice.

### Result Meaning

| Result | Meaning |
| --- | --- |
| Invalid purpose/expiry/binding/generation or inactive account | Generic invalid reset; no recovery writes |
| Confirmed final conditional rejection | Generic invalid reset; no partial recovery commit |
| Confirmed success | Used proof consumed, password replaced and generation incremented together |
| Conflict, outage, timeout, decoding error or uncertain write response | Operational failure, not success or proof of rollback |

Do not retry using freshly loaded snapshots to force a losing reset through. Do not log raw links,
tokens, passwords, PHC hashes or digests. Preserve safe HTTP error mapping and password-policy errors.
A malformed persisted record is a fail-closed storage error, not a compatibility opportunity.

## Make Invalidation Effective

### Authorization Codes And Login

- Capture the active account generation for every password, Google and Apple code producer.
- Code creation includes an account-snapshot condition in the same transaction as the code Put.
- Password code creation additionally conditions on the exact password-user record whose hash was
  verified. Reading the latest account generation after checking an old password must not allow
  that stale password check to mint a new-generation code.
- Code lookup rejects inactive accounts and generation mismatches. Conditional consumption rechecks
  expiry and guards the account snapshot as well as the complete code record.
- Keep client, redirect, PKCE and scope validation before consumption. Do not consume a valid code
  merely because the requester supplied the wrong verifier or client.

### Refresh Issuance And Rotation

- Pass the consumed code's generation into initial refresh issuance; do not fetch the latest
  generation and silently upgrade the old authorization.
- Initial refresh creation and rotation condition their writes on the matching active account.
- Rotation checks token/family generation, subject and client agreement, and carries that same
  generation into the successor. Preserve replay detection and client-bound logout behavior.
- A generation rejection is an invalid credential, not refresh reuse. Keep genuine conditional,
  conflict and operational failures distinct. Any route mapping changes must be limited to these
  new explicit rejection cases, not a blanket conversion of backend errors into `invalid_grant`.
- Preserve slice 03 expiry behavior: renewed family/successor/index deadlines, original spent-token
  deadlines and revocation TTL retention.

No scan or enumeration of every credential is needed for reset invalidation. Other old proofs,
codes and refresh records can remain until their TTL, but generation checks make them unusable.
Unrelated subjects remain unaffected, including their reset links and refresh sessions.

### Honest Concurrency Boundary

The recovery commit is the generation change's ordering point. A guarded credential write that
wins before it belongs to the old generation and becomes unusable afterwards. A stale write trying
to commit after it must fail. A stale lifecycle replacement must not roll the generation backwards.

JWT signing and response delivery are not inside the DynamoDB transaction. An exchange or rotation
authorized before recovery may finish returning an access JWT afterwards. Already-issued access
JWTs remain valid at independently validating resource APIs until expiry; userinfo still follows
its existing account-status policy. Do not claim instant access-JWT/session-cookie invalidation or
that the whole login/token/signing workflow became atomic.

## Expected Change Surface

Paths below are relative to `packages/functions/auth/`:

```text
src/store/recovery.rs                 new recovery operations
src/store/credential_generation.rs    new small guard helpers
src/store/records.rs
src/store/mod.rs                      module wiring, account constructors, narrow clock hook if needed
src/store/password_secrets.rs          retain verification and indexed lifecycle cleanup
src/store/password_users.rs            account initialization, remove independent hash replacement
src/store/authorization_codes.rs
src/store/refresh.rs
src/providers/password.rs
src/api/providers/password.rs          only affected error mapping
src/api/providers/google.rs            generation-bound code producer only
src/api/providers/apple.rs             generation-bound code producer only
src/oauth/token.rs                    propagate original generation and explicit rejection results
```

Existing record constructors in tests must be updated to create valid bound accounts/credentials,
not bypass the guards. In particular inspect password login/reset, token/userinfo, refresh/revoke,
admin lifecycle/deletion and SDK wire/local fixtures. Wire tests may require extra stub responses
for account reads/guards; retain their real SDK request and failure assertions.

Add focused recovery tests under `tests/dynamodb_storage/` and small shared recovery helpers only
where needed. Reuse `tests/support/dynamo.rs` and `transaction_gate.rs`; extend the gate narrowly
for selected read/commit boundaries if a race requires it. Keep new tests domain-named, not numbered.
Do not grow `store/mod.rs` (730 lines), `tests/oauth_refresh_revoke.rs` (709) or
`tests/api_routes/password.rs` (606) into recovery catch-all files. Every source stays below 1,000 lines.

## Required Evidence

### Recovery And Rejection

- Success consumes proof/index once, changes the actual verified password and increments generation
  exactly once. Old-password login fails; new-password login and fresh refresh rotation work.
- Merely requesting recovery changes no credential validity. Successful recovery invalidates other
  outstanding links, unexchanged codes and refresh families across clients for the same subject,
  including a successor rotated before recovery. Another subject's credentials still work.
- Isolate wrong purpose, expired proof, wrong subject/email binding, generation mismatch, missing
  generation, unverified/deleted user, inactive account and weak password. Each case must reach the
  intended rejection rather than being hidden behind another invalid fixture.
- Preparation followed by expiry before finish rejects without consuming the proof. Counter
  overflow rejects without writes. Inspect physical TTL separately from logical expiry.
- A genuine failed condition in the final multi-item transaction leaves the proof/index, hash and
  account unchanged by the rejected transaction. Use an independent read to verify actual state.

### Ordered Races Through DynamoDB Local

Use deterministic gates, not sleeps. Let the real adapter evaluate conditions:

- Two completions of the same link, and of two links at the same generation, produce one commit.
- Recovery versus refresh rotation in both orders cannot leave a usable old-generation successor;
  a rejected rotation leaves no orphan token/index rows or partial TTL changes.
- Recovery versus password login rejects a stale verified-password snapshot, including when
  recovery commits before login reads the account. Guarding only the account is insufficient.
- Recovery versus code consumption or follow-up refresh creation rejects stale writes. A code
  consumed before reset cannot create a new-generation refresh family afterwards.
- Reset-link creation paused before commit cannot write/send a stale link after recovery wins.
- Recovery versus disable/delete in both orders cannot reactivate a user or roll the generation
  back. Confirmed stale lifecycle failure remains visible; a fresh explicit attempt can proceed.

Reuse existing fixtures and assertions where they already establish a case. Bounded exact-key reads
and known-partition queries inspect state; no table scans or deployed tables. Memory tests remain
useful for routes/domain behavior but do not substitute for real transaction evidence.

### Failure Reporting And Regression

Distinguish a test failure before dispatch from a deliberately lost response after a real commit.
The latter must return an operational error even though independent reads show a completed reset.
Neither path should automatically retry or expose sensitive fixture values. Simulated transport
faults demonstrate application handling, not production network reliability.

Retain normal route response, rate-limit, PKCE, client isolation, logout, provider callback,
admin lifecycle, cancellation classification and refresh TTL regressions. Provider fixtures must
exercise their existing callbacks with the new binding, not just deserialize a new record field.
Do not introduce UI tests or live mail/provider/device tests for this backend slice.

## Design And Setup Updates During Implementation

Add exact mirrors for every new file and update every touched source/test mirror, including `src/`
and `tests/`. New recovery and generation modules must state their concrete transaction boundaries.

Update the affected topic docs:

- Accounts/records and password proofs: generation fields, purpose checks and clean data contract.
- Password domain/provider/API: prepare-hash-commit and safe rejection behavior.
- Authorization codes and refresh storage: original-generation propagation and guarded writes.
- Tokens, account lifecycle, overview and threat model: precise reset invalidation and remaining
  in-flight signing, JWT expiry and multi-step lifecycle limitations.
- Testing and the test README: local recovery/race prerequisites and actual evidence.

Review `docs/setup/06-resend-email.md`, `docs/setup/09-deploy-auth-core.md` and
`docs/operations/smoke-test.md` for affected recovery/deployment guidance. State the stored-record
compatibility boundary without inventing a migration workflow. Do not add new provider secrets,
frontend routes or deployment steps unrelated to this change. Remove superseded descriptions
instead of accumulating decision history in authoritative design.

## Out Of Scope

- Registration/verification atomicity, email ownership, shared-account linking or native sign-in.
- New JWT claims, verifier/key rotation, immediate access-token revocation or token introspection.
- Changing disable/enable/revoke-sessions policy to increment generations independently of reset.
- Email-action pages, hosted UI, example apps, SDKs or frontend hosting changes.
- Audit redesign, independent abuse limits, Apple grant revocation or deletion orchestration.
- Complete memory/DynamoDB equivalence, broad backend/test consolidation or unrelated refactors.
- Backwards-compatibility readers, deployed data changes, new infrastructure or deployment.
- Changes to 64x64, Plaza, provider credentials or cloud resources.

## Verification

Before implementation, run the existing password/reset, token/refresh, admin and local-storage
baseline in Irongate. Do not reuse a passing result from 64x64 as port evidence.

After implementation:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
git diff --check
```

Run changed-file rustfmt and exact mirror/link/source-length checks. Separate pre-existing warnings
from regressions. Use the documented disposable loopback fixture with explicit dummy credentials;
stop the local emulator afterwards. An ordinary Cargo run that ignores local cases does not verify
them. Report missing local prerequisites honestly and do not mark the slice complete without its
transaction evidence. Run setup/TypeScript/example builds only if their inputs change.

## Acceptance

- Password hash, used proof/index and generation commit together, with no independent reset bypass.
- Old reset links/codes/refresh tokens cannot authorize new credentials after recovery; new-password
  login and unrelated accounts still work.
- Password snapshot, account/generation and credential conditions survive real competing writes.
- Confirmed rejection, operational failure and uncertain commit remain distinguishable.
- HMAC keys, TTL policy, provider validation, API-only core, IAM isolation and JWT semantics remain.
- Exact design mirrors and operator guidance describe implemented behavior and remaining limits.
- Relevant Irongate regressions and explicit DynamoDB Local recovery tests have actually passed.
- No deployment, reference-repository changes or additional future slice was performed.

## Implementation Result

- Added focused recovery and generation store modules. Required generation fields have no legacy
  fallback. All account constructors initialize zero; code/refresh producers preserve their
  authenticated generation. No runtime dependency, infrastructure or JWT-claim change was needed.
- Reset prepares without consuming, hashes, then commits proof/index deletion, exact user replacement
  and checked account increment together. Removed the independent consume/hash-update APIs.
- Guarded password/provider code creation, code consumption and refresh creation/rotation. Password
  issuance also checks the exact verified user snapshot. Generation rejection is not replay;
  operational and uncertain failures retain their distinct meaning.
- Added real local recovery/rejection/fault/race cases using existing disposable fixtures. Existing
  route/provider/admin regressions exercise the new bindings. An HTTP test confirms old code/refresh
  invalid_grant responses while an already-issued access JWT still verifies.
- Updated all 27 touched source/test/README mirrors, relevant topic docs and setup/smoke guidance.
  Reference behavior was adapted from the pinned files above without product or deployment values.

### Verification Results

Commands used `--locked --offline` for Rust checks with existing local dependencies.

| Check | Result |
| --- | --- |
| Focused pre-change password/token/refresh/admin baseline | Passed |
| Pre-change DynamoDB Local baseline | 10 passed |
| Full auth Cargo tests, including existing Google/Apple callbacks and doc tests | 241 passed, 22 local cases explicitly ignored by ordinary run |
| Explicit DynamoDB Local suite after implementation and final fixture cleanup | 22 passed, including 12 recovery/rejection/race cases |
| Final password-reset, refresh/revoke and admin-lifecycle targets | Passed |
| Auth and admin cargo check | Passed |
| Auth Clippy, all targets | Completed; existing style warnings remain, new gate warnings fixed |
| Changed-file rustfmt | Passed |
| npm run test:infra | Passed |
| Exact mirrors, changed-document links and source length | Passed; no touched source reaches 1,000 lines |
| git diff --check | Passed |

The emulator used Java 17 and DynamoDB Local 3.3.2, in-memory with dummy credentials on loopback
port 18023. Fixtures created and removed their own unique tables. Actual backend conditions and
independent raw reads established rollback/TTL results. Fault injection distinguished pre-dispatch
failure from a response lost after a real commit; it is not production network certification.

TypeScript/setup/example/native checks were not run because their inputs did not change. No live
email/provider/device/UI tests, AWS/Cloudflare deployment, reference-repository edit or Plaza change
was performed. Existing populated stages still require a separately approved data decision for
required generation fields; there is no automatic migration. Access-JWT expiry, in-flight signing
and multi-step lifecycle limits remain as documented.
